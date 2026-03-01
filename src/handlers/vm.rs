use crate::config::{validate_vm_config, InterfaceConfig, NetworkType, VMConfig, VMConfigBuilder};
use crate::kube::types::VirtualMachine;
use crate::output::{format_output, OutputFormat};
use crate::templates::TEMPLATES;
use crate::tui::colors::cli as color;
use anyhow::{anyhow, Result};

/// Convert a KubeVirt VirtualMachine to a VMConfig
fn vm_to_config(vm: &VirtualMachine, namespace: &str) -> VMConfig {
    let name = vm.metadata.name.clone().unwrap_or_default();
    let spec = &vm.spec.template.spec;
    let domain = &spec.domain;

    let cpu_cores = domain.cpu.as_ref().and_then(|c| c.cores).unwrap_or(1);
    let cpu_sockets = domain.cpu.as_ref().and_then(|c| c.sockets).unwrap_or(1);
    let cpu_threads = domain.cpu.as_ref().and_then(|c| c.threads).unwrap_or(1);

    let memory = domain
        .memory
        .as_ref()
        .and_then(|m| m.guest.as_deref())
        .or_else(|| {
            domain
                .resources
                .requests
                .as_ref()
                .and_then(|r| r.get("memory"))
                .map(|s| s.as_str())
        })
        .unwrap_or("2Gi")
        .to_string();

    let mut builder = VMConfigBuilder::new(&name)
        .namespace(namespace)
        .cpu(cpu_cores, cpu_sockets, cpu_threads)
        .memory(&memory);

    // Extract CPU model
    if let Some(ref cpu) = domain.cpu {
        if let Some(ref model) = cpu.model {
            builder = builder.cpu_model(model);
        }
    }

    // Extract disks from volumes
    if let Some(ref volumes) = spec.volumes {
        for (i, vol) in volumes.iter().enumerate() {
            if let Some(ref container_disk) = vol.container_disk {
                builder =
                    builder.add_container_disk(&vol.name, &container_disk.image, (i as u32).saturating_add(1));
            } else if let Some(ref pvc) = vol.persistent_volume_claim {
                builder = builder.add_disk(crate::config::DiskConfig {
                    name: vol.name.clone(),
                    size: "0".to_string(),
                    storage_class: None,
                    boot_order: (i as u32).saturating_add(1),
                    source: crate::config::DiskSource::PVC {
                        name: pvc.claim_name.clone(),
                    },
                });
            } else if let Some(ref empty) = vol.empty_disk {
                builder = builder.add_blank_disk(&vol.name, &empty.capacity, (i as u32).saturating_add(1));
            } else if let Some(ref cloud_init) = vol.cloud_init_no_cloud {
                if let Some(ref user_data) = cloud_init.user_data {
                    builder = builder.cloud_init(user_data);
                }
            }
        }
    }

    // Extract network interfaces
    if let Some(ref devices) = domain.devices {
        if let Some(ref ifaces) = devices.interfaces {
            for iface in ifaces {
                let network_type = if let Some(ref networks) = spec.networks {
                    networks
                        .iter()
                        .find(|n| n.name == iface.name)
                        .map(|n| {
                            if let Some(ref multus) = n.multus {
                                NetworkType::Multus {
                                    name: multus.network_name.clone(),
                                }
                            } else if n.pod.is_some() {
                                NetworkType::Pod
                            } else {
                                NetworkType::Bridge
                            }
                        })
                        .unwrap_or(NetworkType::Pod)
                } else {
                    NetworkType::Pod
                };

                builder = builder.add_interface(InterfaceConfig {
                    name: iface.name.clone(),
                    network: iface.name.clone(),
                    model: iface.model.clone().unwrap_or_else(|| "virtio".to_string()),
                    network_type,
                });
            }
        }
    }

    // Extract labels
    if let Some(ref labels) = vm.metadata.labels {
        for (k, v) in labels {
            builder = builder.label(k, v);
        }
    }

    builder.build()
}

fn load_or_create_config(
    name: &str,
    namespace: &str,
    template: Option<String>,
    from_file: Option<String>,
) -> Result<VMConfig> {
    use std::fs;

    if let Some(file) = from_file {
        let content = fs::read_to_string(&file)?;
        let mut config: VMConfig = if file.ends_with(".json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };
        config.name = name.to_string();
        config.namespace = namespace.to_string();
        Ok(config)
    } else if let Some(template_name) = template {
        let mut config = TEMPLATES
            .get(&template_name)
            .ok_or_else(|| anyhow!("Template not found: {}", template_name))?;
        config.name = name.to_string();
        config.namespace = namespace.to_string();
        Ok(config)
    } else {
        // Create a basic default VM
        Ok(VMConfigBuilder::new(name)
            .namespace(namespace)
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .add_pod_network("default")
            .build())
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_create(
    name: String,
    template: Option<String>,
    from_file: Option<String>,
    cpus: Option<u32>,
    memory: Option<String>,
    disk_size: Option<String>,
    cloud_init: Option<String>,
    dry_run: bool,
    output: String,
    namespace: &str,
) -> Result<()> {
    use crate::kube;
    use std::fs;

    let mut config = load_or_create_config(&name, namespace, template, from_file)?;

    // Apply CLI overrides
    if let Some(cpus) = cpus {
        config.cpu.cores = cpus;
    }
    if let Some(memory) = memory {
        config.memory.size = memory;
    }
    if let Some(disk_size) = disk_size {
        if let Some(disk) = config.disks.first_mut() {
            disk.size = disk_size;
        }
    }
    if let Some(cloud_init_file) = cloud_init {
        let user_data = fs::read_to_string(&cloud_init_file)?;
        config.cloud_init = Some(crate::config::CloudInitConfig {
            user_data,
            network_data: None,
        });
    }

    // Validate configuration
    validate_vm_config(&config)?;

    let format = OutputFormat::parse_format(&output)
        .ok_or_else(|| anyhow!("Invalid output format: {}", output))?;

    if dry_run {
        println!("# Dry run - VM manifest:");
        let kubevirt_vm = kube::vm_config_to_kubevirt(&config)?;
        let output = format_output(&kubevirt_vm, format)?;
        println!("{}", output);
    } else {
        log::info!("Creating VM: {}", name);

        // Create the VM via Kubernetes API
        match kube::KubeClient::new().await {
            Ok(client) => match client.create_vm(&config).await {
                Ok(_vm) => {
                    println!(
                        "{}",
                        color::success(&format!("VM '{}' created successfully", name))
                    );
                    println!("  Namespace: {}", color::namespace(&config.namespace));
                    println!(
                        "  Status: {} (use '{}' to start)",
                        color::vm_status("Stopped"),
                        color::command(&format!("zorvia start {}", name))
                    );
                }
                Err(e) => {
                    return Err(anyhow!("Failed to create VM: {}", e));
                }
            },
            Err(e) => {
                return Err(anyhow!("Failed to connect to Kubernetes: {}", e));
            }
        }
    }
    Ok(())
}

pub async fn handle_list(all_namespaces: bool, output: String, namespace: &str) -> Result<()> {
    use crate::kube;
    use crate::output;

    let client = kube::KubeClient::new().await?;

    let vms = if all_namespaces {
        client.list_all_vms().await?
    } else {
        client.list_vms(namespace).await?
    };

    if vms.is_empty() {
        println!("No VMs found");
        return Ok(());
    }

    match output.as_str() {
        "yaml" => {
            let yaml = output::to_yaml(&vms)?;
            println!("{}", yaml);
        }
        "json" => {
            let json = output::to_json(&vms)?;
            println!("{}", json);
        }
        _ => {
            use crate::tui::colors::vm_status_symbol;

            // Print header with theme colors
            println!(
                "{:<30} {:<20} {:<15} {:<10}",
                color::header("NAME"),
                color::header("NAMESPACE"),
                color::header("STATUS"),
                color::header("RUNNING")
            );
            println!("{}", color::muted(&"-".repeat(75)));

            for vm in vms {
                let name = vm.metadata.name.as_deref().unwrap_or("N/A");
                let namespace = vm.metadata.namespace.as_deref().unwrap_or("N/A");
                let running = if vm.spec.running.unwrap_or(false) {
                    color::success("Yes")
                } else {
                    color::muted("No")
                };
                let status = if let Some(s) = &vm.status {
                    s.print_able_status.as_deref().unwrap_or("Unknown")
                } else {
                    "Unknown"
                };

                // Format with theme colors
                let status_display =
                    format!("{} {}", vm_status_symbol(status), color::vm_status(status));

                println!(
                    "{:<30} {:<20} {:<25} {:<10}",
                    color::vm_name(name),
                    color::namespace(namespace),
                    status_display,
                    running
                );
            }
        }
    }
    Ok(())
}

pub async fn handle_get(name: String, output: String, namespace: &str) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;
    let vm = client.get_vm(namespace, &name).await?;

    let format = OutputFormat::parse_format(&output)
        .ok_or_else(|| anyhow!("Invalid output format: {}", output))?;

    let formatted = format_output(&vm, format)?;
    println!("{}", formatted);
    Ok(())
}

pub async fn handle_delete(name: String, yes: bool, namespace: &str) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;

    if !yes {
        use std::io::Write;
        print!("Are you sure you want to delete VM '{}'? (y/N): ", name);
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    client.delete_vm(namespace, &name).await?;
    println!(
        "{}",
        color::success(&format!("VM '{}' deleted successfully", name))
    );
    Ok(())
}

pub async fn handle_start(name: String, namespace: &str) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;
    client.start_vm(namespace, &name).await?;
    println!(
        "{}",
        color::success(&format!("VM '{}' started successfully", name))
    );
    Ok(())
}

pub async fn handle_stop(name: String, namespace: &str) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;
    client.stop_vm(namespace, &name).await?;
    println!(
        "{}",
        color::success(&format!("VM '{}' stopped successfully", name))
    );
    Ok(())
}

pub async fn handle_restart(name: String, namespace: &str) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;
    println!("{}", color::info(&format!("Restarting VM '{}'...", name)));
    client.restart_vm(namespace, &name).await?;
    println!(
        "{}",
        color::success(&format!("VM '{}' restarted successfully", name))
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn handle_generate(
    name: String,
    template: Option<String>,
    from_file: Option<String>,
    cpus: Option<u32>,
    memory: Option<String>,
    disk_size: Option<String>,
    output: Option<String>,
    format: String,
    kubevirt: bool,
    namespace: &str,
) -> Result<()> {
    use crate::kube;
    use std::fs;

    let mut config = load_or_create_config(&name, namespace, template, from_file)?;

    // Apply CLI overrides
    if let Some(cpus) = cpus {
        config.cpu.cores = cpus;
    }
    if let Some(memory) = memory {
        config.memory.size = memory;
    }
    if let Some(disk_size) = disk_size {
        if let Some(disk) = config.disks.first_mut() {
            disk.size = disk_size;
        }
    }

    // Validate configuration
    validate_vm_config(&config)?;

    let output_format = OutputFormat::parse_format(&format)
        .ok_or_else(|| anyhow!("Invalid output format: {}", format))?;

    let manifest = if kubevirt {
        // Convert to KubeVirt VirtualMachine CRD
        let kubevirt_vm = kube::vm_config_to_kubevirt(&config)?;
        format_output(&kubevirt_vm, output_format)?
    } else {
        // Output VMConfig format
        format_output(&config, output_format)?
    };

    if let Some(output_file) = output {
        fs::write(&output_file, &manifest)?;
        println!("Manifest written to: {}", output_file);
    } else {
        println!("{}", manifest);
    }
    Ok(())
}

pub fn handle_templates() -> Result<()> {
    println!("{}", color::header("Available templates:"));
    for template in TEMPLATES.list() {
        println!("  {} {}", color::value("•"), color::label(&template));
    }
    Ok(())
}

pub fn handle_template(name: String, output: String) -> Result<()> {
    let config = TEMPLATES
        .get(&name)
        .ok_or_else(|| anyhow!("Template not found: {}", name))?;

    let format = OutputFormat::parse_format(&output)
        .ok_or_else(|| anyhow!("Invalid output format: {}", output))?;

    let manifest = format_output(&config, format)?;
    println!("{}", manifest);
    Ok(())
}

pub fn handle_validate(file: String) -> Result<()> {
    use std::fs;

    let content = fs::read_to_string(&file)?;
    let config: VMConfig = if file.ends_with(".json") {
        serde_json::from_str(&content)?
    } else {
        serde_yaml::from_str(&content)?
    };

    validate_vm_config(&config)?;
    println!("{}", color::success("Configuration is valid"));
    Ok(())
}

pub async fn handle_status(
    name: String,
    watch: bool,
    interval: u64,
    namespace: &str,
) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;

    if watch {
        loop {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");

            match client.get_vm(namespace, &name).await {
                Ok(vm) => {
                    let status = kube::VMStatus::from_vm(&vm);
                    status.display();
                }
                Err(e) => {
                    eprintln!("Error fetching VM status: {}", e);
                    break;
                }
            }

            println!();
            println!("Press Ctrl+C to exit watch mode...");
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    } else {
        let vm = client.get_vm(namespace, &name).await?;
        let status = kube::VMStatus::from_vm(&vm);
        status.display();
    }
    Ok(())
}

pub async fn handle_clone(
    source: String,
    target: String,
    start: bool,
    namespace: &str,
) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;

    println!(
        "{}",
        color::info(&format!("Cloning VM '{}' to '{}'...", source, target))
    );

    // Get source VM
    let source_vm = client.get_vm(namespace, &source).await?;

    // Convert to VMConfig
    let cpu_ref = source_vm
        .spec
        .template
        .spec
        .domain
        .cpu
        .as_ref()
        .ok_or_else(|| anyhow!("Source VM has no CPU configuration"))?;
    let mem_ref = source_vm
        .spec
        .template
        .spec
        .domain
        .memory
        .as_ref()
        .ok_or_else(|| anyhow!("Source VM has no memory configuration"))?;
    let guest_mem = mem_ref
        .guest
        .as_ref()
        .ok_or_else(|| anyhow!("Source VM has no guest memory configuration"))?;

    let mut config = VMConfigBuilder::new(&target)
        .namespace(namespace)
        .cpu(
            cpu_ref.cores.unwrap_or(1),
            cpu_ref.sockets.unwrap_or(1),
            cpu_ref.threads.unwrap_or(1),
        )
        .memory(guest_mem.clone())
        .build();

    // Copy labels (but update the name label)
    if let Some(labels) = source_vm.metadata.labels {
        for (k, v) in labels {
            if k != "kubevirt.io/vm" {
                config.labels.insert(k, v);
            }
        }
    }

    // Create the cloned VM
    client.create_vm(&config).await?;
    println!(
        "{}",
        color::success(&format!("VM '{}' cloned successfully", target))
    );

    if start {
        println!("{}", color::info(&format!("Starting VM '{}'...", target)));
        client.start_vm(namespace, &target).await?;
        println!("{}", color::success(&format!("VM '{}' started", target)));
    }
    Ok(())
}

pub async fn handle_resources(
    all_namespaces: bool,
    sort_by: String,
    namespace: &str,
) -> Result<()> {
    use crate::kube;

    let client = kube::KubeClient::new().await?;

    let vms = if all_namespaces {
        client.list_all_vms().await?
    } else {
        client.list_vms(namespace).await?
    };

    let summary = kube::ResourceSummary::from_vms(&vms);
    summary.display();

    println!();
    println!(
        "{}",
        color::header("╔═══════════════════════════════════════════════════════════════╗")
    );
    println!(
        "{}",
        color::header("║                   VM Resource Details                         ║")
    );
    println!(
        "{}",
        color::header("╚═══════════════════════════════════════════════════════════════╝")
    );
    println!();
    println!(
        "{:<30} {:<15} {:<10} {:<10}",
        color::header("NAME"),
        color::header("NAMESPACE"),
        color::header("CPU"),
        color::header("MEMORY")
    );
    println!("{}", color::muted(&"-".repeat(70)));

    let mut vm_infos: Vec<_> = vms
        .iter()
        .map(|vm| {
            let name = vm.metadata.name.as_deref().unwrap_or("N/A");
            let namespace = vm.metadata.namespace.as_deref().unwrap_or("N/A");
            let cpu = vm
                .spec
                .template
                .spec
                .domain
                .cpu
                .as_ref()
                .and_then(|c| c.cores)
                .unwrap_or(0);
            let memory = vm
                .spec
                .template
                .spec
                .domain
                .memory
                .as_ref()
                .and_then(|m| m.guest.as_deref())
                .unwrap_or("N/A");
            (name, namespace, cpu, memory)
        })
        .collect();

    // Sort
    match sort_by.as_str() {
        "cpu" => vm_infos.sort_by_key(|&(_, _, cpu, _)| std::cmp::Reverse(cpu)),
        "memory" => vm_infos.sort_by_key(|&(_, _, _, mem)| std::cmp::Reverse(mem.to_string())),
        _ => vm_infos.sort_by_key(|&(name, _, _, _)| name),
    }

    for (name, namespace, cpu, memory) in vm_infos {
        println!(
            "{:<30} {:<15} {:<10} {:<10}",
            color::vm_name(name),
            color::namespace(namespace),
            color::resource(&cpu.to_string(), "cpu"),
            color::resource(memory, "memory")
        );
    }
    Ok(())
}

pub async fn handle_export(
    name: String,
    output: Option<String>,
    kubevirt: bool,
    namespace: &str,
) -> Result<()> {
    use crate::kube;
    use crate::output;
    use std::fs;

    let client = kube::KubeClient::new().await?;
    let vm = client.get_vm(namespace, &name).await?;

    let manifest = if kubevirt {
        output::to_yaml(&vm)?
    } else {
        // Convert KubeVirt VM to VMConfig format
        let config = vm_to_config(&vm, namespace);
        output::to_yaml(&config)?
    };

    if let Some(output_file) = output {
        fs::write(&output_file, &manifest)?;
        println!(
            "{}",
            color::success(&format!("VM exported to: {}", color::path(&output_file)))
        );
    } else {
        println!("{}", manifest);
    }
    Ok(())
}

pub async fn handle_wizard(name: Option<String>, namespace: &str) -> Result<()> {
    use crate::kube;
    use dialoguer::{Input, Select};

    println!(
        "{}",
        color::header("╔═══════════════════════════════════════════════════════════════╗")
    );
    println!(
        "{}",
        color::header("║           Interactive VM Creation Wizard                      ║")
    );
    println!(
        "{}",
        color::header("╚═══════════════════════════════════════════════════════════════╝")
    );
    println!();

    // VM Name
    let vm_name: String = if let Some(n) = name {
        n
    } else {
        Input::new().with_prompt("VM Name").interact_text()?
    };

    // Template selection
    let templates = TEMPLATES.list();
    let template_idx = Select::new()
        .with_prompt("Select a template")
        .items(&templates)
        .default(0)
        .interact()?;
    let template_name = &templates[template_idx];

    // CPU Cores
    let cpu_cores: u32 = Input::new()
        .with_prompt("CPU Cores")
        .default(2)
        .interact_text()?;

    // Memory
    let memory: String = Input::new()
        .with_prompt("Memory (e.g., 4Gi, 8Gi)")
        .default("4Gi".to_string())
        .interact_text()?;

    // Disk Size
    let disk_size: String = Input::new()
        .with_prompt("Disk Size (e.g., 20Gi, 40Gi)")
        .default("20Gi".to_string())
        .interact_text()?;

    // Start immediately?
    let start_vm = Select::new()
        .with_prompt("Start VM immediately?")
        .items(&["No", "Yes"])
        .default(0)
        .interact()?
        == 1;

    println!();
    println!(
        "{}",
        color::info("Creating VM with the following configuration:")
    );
    println!("  {:<12} {}", color::label("Name:"), color::value(&vm_name));
    println!(
        "  {:<12} {}",
        color::label("Template:"),
        color::value(template_name)
    );
    println!(
        "  {:<12} {}",
        color::label("CPU:"),
        color::resource(&format!("{} cores", cpu_cores), "cpu")
    );
    println!(
        "  {:<12} {}",
        color::label("Memory:"),
        color::resource(&memory, "memory")
    );
    println!(
        "  {:<12} {}",
        color::label("Disk:"),
        color::resource(&disk_size, "disk")
    );
    println!();

    // Create VM
    let mut config = TEMPLATES
        .get(template_name)
        .ok_or_else(|| anyhow!("Unknown template: {}", template_name))?;
    config.name = vm_name.clone();
    config.namespace = namespace.to_string();
    config.cpu.cores = cpu_cores;
    config.memory.size = memory;
    if let Some(disk) = config.disks.first_mut() {
        disk.size = disk_size;
    }

    validate_vm_config(&config)?;

    let client = kube::KubeClient::new().await?;
    client.create_vm(&config).await?;
    println!(
        "{}",
        color::success(&format!("VM '{}' created successfully", vm_name))
    );

    if start_vm {
        client.start_vm(namespace, &vm_name).await?;
        println!("{}", color::success(&format!("VM '{}' started", vm_name)));
    }
    Ok(())
}

pub async fn handle_batch(
    file: String,
    namespace_override: Option<String>,
    dry_run: bool,
    continue_on_error: bool,
    namespace: &str,
) -> Result<()> {
    use crate::kube;
    use crate::utils;
    use indicatif::{ProgressBar, ProgressStyle};

    println!(
        "{}",
        color::info(&format!(
            "Loading batch configuration from: {}",
            color::path(&file)
        ))
    );
    let mut batch = utils::BatchConfig::from_file(&file)?;

    // Apply namespace override
    if let Some(ns) = namespace_override {
        batch.apply_namespace(&ns);
    } else {
        batch.apply_namespace(namespace);
    }

    println!(
        "{}",
        color::info(&format!("Found {} VMs to create", batch.vms.len()))
    );
    println!();

    if dry_run {
        println!("{}", color::info("Dry run - VMs that would be created:"));
        for (i, vm) in batch.vms.iter().enumerate() {
            println!(
                "  {}. {} (namespace: {}, {} cores, {})",
                color::value(&(i + 1).to_string()),
                color::vm_name(&vm.name),
                color::namespace(&vm.namespace),
                color::resource(&vm.cpu.cores.to_string(), "cpu"),
                color::resource(&vm.memory.size, "memory")
            );
        }
        return Ok(());
    }

    let client = kube::KubeClient::new().await?;
    let pb = ProgressBar::new(batch.vms.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .expect("invalid progress bar template")
            .progress_chars("=>-"),
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for vm_config in batch.vms.iter() {
        pb.set_message(format!("Creating {}", vm_config.name));

        // Validate
        if let Err(e) = validate_vm_config(vm_config) {
            eprintln!("✗ Validation failed for '{}': {}", vm_config.name, e);
            error_count += 1;

            if !continue_on_error {
                pb.abandon();
                return Err(e);
            }

            pb.inc(1);
            continue;
        }

        // Create
        match client.create_vm(vm_config).await {
            Ok(_) => {
                success_count += 1;
                pb.println(format!("  ✓ {} created successfully", vm_config.name));
            }
            Err(e) => {
                error_count += 1;
                pb.println(format!("  ✗ {} failed: {}", vm_config.name, e));

                if !continue_on_error {
                    pb.abandon();
                    return Err(e);
                }
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Batch creation complete");

    println!();
    println!(
        "{}",
        color::header("╔═══════════════════════════════════════════════════════════════╗")
    );
    println!(
        "{}",
        color::header("║                   Batch Summary                               ║")
    );
    println!(
        "{}",
        color::header("╚═══════════════════════════════════════════════════════════════╝")
    );
    println!(
        "  {:<12} {}",
        color::label("Total VMs:"),
        color::value(&batch.vms.len().to_string())
    );
    println!(
        "  {:<12} {}",
        color::label("Successful:"),
        color::success(&format!("{} ✓", success_count))
    );
    println!(
        "  {:<12} {}",
        color::label("Failed:"),
        if error_count > 0 {
            color::error(&format!("{} ✗", error_count))
        } else {
            color::muted(&format!("{} ✗", error_count))
        }
    );

    if error_count > 0 && !continue_on_error {
        return Err(anyhow!("Batch creation had {} error(s)", error_count));
    }
    Ok(())
}
