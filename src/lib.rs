pub mod cli;
pub mod config;
pub mod kube;
pub mod network;
pub mod output;
pub mod storage;
pub mod templates;
pub mod utils;

use anyhow::{anyhow, Result};
use cli::{Cli, Commands};
use config::{validate_vm_config, VMConfig, VMConfigBuilder};
use output::{format_output, OutputFormat};
use std::fs;
use templates::TEMPLATES;

/// Main entry point for the library
pub async fn run(cli: Cli) -> Result<()> {
    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    match cli.command {
        Commands::Create {
            name,
            template,
            from_file,
            cpus,
            memory,
            disk_size,
            storage_class: _,
            container_disk: _,
            cloud_init,
            dry_run,
            output,
        } => {
            let mut config = load_or_create_config(&name, &cli.namespace, template, from_file)?;

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
                config.cloud_init = Some(config::CloudInitConfig {
                    user_data,
                    network_data: None,
                });
            }

            // Validate configuration
            validate_vm_config(&config)?;

            let format = OutputFormat::from_str(&output)
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
                    Ok(client) => {
                        match client.create_vm(&config).await {
                            Ok(_vm) => {
                                println!("✓ VM '{}' created successfully", name);
                                println!("  Namespace: {}", config.namespace);
                                println!("  Status: Stopped (use 'zorvia start {}' to start)", name);
                            }
                            Err(e) => {
                                eprintln!("✗ Failed to create VM: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to connect to Kubernetes: {}", e);
                        eprintln!("  Make sure kubectl is configured and you have access to the cluster");
                        std::process::exit(1);
                    }
                }
            }
        }

        Commands::List {
            all_namespaces,
            output,
        } => {
            let client = kube::KubeClient::new().await?;

            let vms = if all_namespaces {
                client.list_all_vms().await?
            } else {
                client.list_vms(&cli.namespace).await?
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
                "table" | _ => {
                    println!("{:<30} {:<20} {:<10} {:<10}", "NAME", "NAMESPACE", "STATUS", "RUNNING");
                    println!("{}", "-".repeat(70));
                    for vm in vms {
                        let name = vm.metadata.name.as_deref().unwrap_or("N/A");
                        let namespace = vm.metadata.namespace.as_deref().unwrap_or("N/A");
                        let running = if vm.spec.running.unwrap_or(false) {
                            "Yes"
                        } else {
                            "No"
                        };
                        let status = if let Some(s) = &vm.status {
                            s.print_able_status.as_deref().unwrap_or("Unknown")
                        } else {
                            "Unknown"
                        };
                        println!("{:<30} {:<20} {:<10} {:<10}", name, namespace, status, running);
                    }
                }
            }
        }

        Commands::Get { name, output } => {
            let client = kube::KubeClient::new().await?;
            let vm = client.get_vm(&cli.namespace, &name).await?;

            let format = OutputFormat::from_str(&output)
                .ok_or_else(|| anyhow!("Invalid output format: {}", output))?;

            let formatted = format_output(&vm, format)?;
            println!("{}", formatted);
        }

        Commands::Delete { name, yes } => {
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

            client.delete_vm(&cli.namespace, &name).await?;
            println!("✓ VM '{}' deleted successfully", name);
        }

        Commands::Start { name } => {
            let client = kube::KubeClient::new().await?;
            client.start_vm(&cli.namespace, &name).await?;
            println!("✓ VM '{}' started successfully", name);
        }

        Commands::Stop { name } => {
            let client = kube::KubeClient::new().await?;
            client.stop_vm(&cli.namespace, &name).await?;
            println!("✓ VM '{}' stopped successfully", name);
        }

        Commands::Restart { name } => {
            let client = kube::KubeClient::new().await?;
            println!("Restarting VM '{}'...", name);
            client.restart_vm(&cli.namespace, &name).await?;
            println!("✓ VM '{}' restarted successfully", name);
        }

        Commands::Generate {
            name,
            template,
            from_file,
            cpus,
            memory,
            disk_size,
            output,
            format,
            kubevirt,
        } => {
            let mut config =
                load_or_create_config(&name, &cli.namespace, template, from_file)?;

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

            let output_format = OutputFormat::from_str(&format)
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
        }

        Commands::Templates => {
            println!("Available templates:");
            for template in TEMPLATES.list() {
                println!("  - {}", template);
            }
        }

        Commands::Template { name, output } => {
            let config = TEMPLATES
                .get(&name)
                .ok_or_else(|| anyhow!("Template not found: {}", name))?;

            let format = OutputFormat::from_str(&output)
                .ok_or_else(|| anyhow!("Invalid output format: {}", output))?;

            let manifest = format_output(&config, format)?;
            println!("{}", manifest);
        }

        Commands::Validate { file } => {
            let content = fs::read_to_string(&file)?;
            let config: VMConfig = if file.ends_with(".json") {
                serde_json::from_str(&content)?
            } else {
                serde_yaml::from_str(&content)?
            };

            validate_vm_config(&config)?;
            println!("✓ Configuration is valid");
        }

        Commands::Status {
            name,
            watch,
            interval,
        } => {
            let client = kube::KubeClient::new().await?;

            if watch {
                loop {
                    // Clear screen
                    print!("\x1B[2J\x1B[1;1H");

                    match client.get_vm(&cli.namespace, &name).await {
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
                let vm = client.get_vm(&cli.namespace, &name).await?;
                let status = kube::VMStatus::from_vm(&vm);
                status.display();
            }
        }

        Commands::Clone {
            source,
            target,
            start,
        } => {
            let client = kube::KubeClient::new().await?;

            println!("Cloning VM '{}' to '{}'...", source, target);

            // Get source VM
            let source_vm = client.get_vm(&cli.namespace, &source).await?;

            // Convert to VMConfig
            let mut config = VMConfigBuilder::new(&target)
                .namespace(&cli.namespace)
                .cpu(
                    source_vm.spec.template.spec.domain.cpu.as_ref().unwrap().cores.unwrap(),
                    source_vm.spec.template.spec.domain.cpu.as_ref().unwrap().sockets.unwrap(),
                    source_vm.spec.template.spec.domain.cpu.as_ref().unwrap().threads.unwrap(),
                )
                .memory(
                    source_vm
                        .spec
                        .template
                        .spec
                        .domain
                        .memory
                        .as_ref()
                        .unwrap()
                        .guest
                        .as_ref()
                        .unwrap()
                        .clone(),
                )
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
            println!("✓ VM '{}' cloned successfully", target);

            if start {
                println!("Starting VM '{}'...", target);
                client.start_vm(&cli.namespace, &target).await?;
                println!("✓ VM '{}' started", target);
            }
        }

        Commands::Resources {
            all_namespaces,
            sort_by,
        } => {
            let client = kube::KubeClient::new().await?;

            let vms = if all_namespaces {
                client.list_all_vms().await?
            } else {
                client.list_vms(&cli.namespace).await?
            };

            let summary = kube::ResourceSummary::from_vms(&vms);
            summary.display();

            println!();
            println!("╔═══════════════════════════════════════════════════════════════╗");
            println!("║                   VM Resource Details                         ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
            println!();
            println!("{:<30} {:<15} {:<10} {:<10}", "NAME", "NAMESPACE", "CPU", "MEMORY");
            println!("{}", "-".repeat(70));

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
                println!("{:<30} {:<15} {:<10} {:<10}", name, namespace, cpu, memory);
            }
        }

        Commands::Export {
            name,
            output,
            kubevirt,
        } => {
            let client = kube::KubeClient::new().await?;
            let vm = client.get_vm(&cli.namespace, &name).await?;

            let manifest = if kubevirt {
                output::to_yaml(&vm)?
            } else {
                // Convert KubeVirt VM back to VMConfig would require reverse conversion
                // For now, just export the KubeVirt format
                output::to_yaml(&vm)?
            };

            if let Some(output_file) = output {
                fs::write(&output_file, &manifest)?;
                println!("✓ VM exported to: {}", output_file);
            } else {
                println!("{}", manifest);
            }
        }

        Commands::Wizard { name } => {
            println!("╔═══════════════════════════════════════════════════════════════╗");
            println!("║           Interactive VM Creation Wizard                      ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
            println!();

            use dialoguer::{Input, Select};

            // VM Name
            let vm_name: String = if let Some(n) = name {
                n
            } else {
                Input::new()
                    .with_prompt("VM Name")
                    .interact_text()?
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
                .interact()? == 1;

            println!();
            println!("Creating VM with the following configuration:");
            println!("  Name:     {}", vm_name);
            println!("  Template: {}", template_name);
            println!("  CPU:      {} cores", cpu_cores);
            println!("  Memory:   {}", memory);
            println!("  Disk:     {}", disk_size);
            println!();

            // Create VM
            let mut config = TEMPLATES.get(template_name).unwrap();
            config.name = vm_name.clone();
            config.namespace = cli.namespace.clone();
            config.cpu.cores = cpu_cores;
            config.memory.size = memory;
            if let Some(disk) = config.disks.first_mut() {
                disk.size = disk_size;
            }

            validate_vm_config(&config)?;

            let client = kube::KubeClient::new().await?;
            client.create_vm(&config).await?;
            println!("✓ VM '{}' created successfully", vm_name);

            if start_vm {
                client.start_vm(&cli.namespace, &vm_name).await?;
                println!("✓ VM '{}' started", vm_name);
            }
        }

        Commands::Batch {
            file,
            namespace,
            dry_run,
            continue_on_error,
        } => {
            use indicatif::{ProgressBar, ProgressStyle};

            println!("Loading batch configuration from: {}", file);
            let mut batch = utils::BatchConfig::from_file(&file)?;

            // Apply namespace override
            if let Some(ns) = namespace {
                batch.apply_namespace(&ns);
            } else {
                batch.apply_namespace(&cli.namespace);
            }

            println!("Found {} VMs to create", batch.vms.len());
            println!();

            if dry_run {
                println!("Dry run - VMs that would be created:");
                for (i, vm) in batch.vms.iter().enumerate() {
                    println!("  {}. {} (namespace: {}, {}cores, {})",
                        i + 1, vm.name, vm.namespace, vm.cpu.cores, vm.memory.size);
                }
                return Ok(());
            }

            let client = kube::KubeClient::new().await?;
            let pb = ProgressBar::new(batch.vms.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                    .unwrap()
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
            println!("╔═══════════════════════════════════════════════════════════════╗");
            println!("║                   Batch Summary                               ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
            println!("  Total VMs:    {}", batch.vms.len());
            println!("  Successful:   {} ✓", success_count);
            println!("  Failed:       {} ✗", error_count);

            if error_count > 0 && !continue_on_error {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn load_or_create_config(
    name: &str,
    namespace: &str,
    template: Option<String>,
    from_file: Option<String>,
) -> Result<VMConfig> {
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
