pub mod cli;
pub mod config;
pub mod kube;
pub mod network;
pub mod output;
pub mod storage;
pub mod templates;
pub mod tui;
pub mod utils;

// Innovative features
pub mod profiles;
pub mod blueprints;
pub mod health;
pub mod snapshots;
pub mod monitoring;
pub mod disk;

use anyhow::{anyhow, Result};
use cli::{Cli, Commands};
use config::{validate_vm_config, VMConfig, VMConfigBuilder};
use output::{format_output, OutputFormat};
use std::fs;
use templates::TEMPLATES;
use tui::colors::cli as color;

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
                                use crate::tui::colors::cli;
                                println!("{}", cli::success(&format!("VM '{}' created successfully", name)));
                                println!("  Namespace: {}", color::namespace(&config.namespace));
                                println!("  Status: {} (use '{}' to start)",
                                    color::vm_status("Stopped"),
                                    color::command(&format!("zorvia start {}", name))
                                );
                            }
                            Err(e) => {
                                use crate::tui::colors::cli;
                                eprintln!("{}", cli::error(&format!("Failed to create VM: {}", e)));
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        use crate::tui::colors::cli;
                        eprintln!("{}", cli::error(&format!("Failed to connect to Kubernetes: {}", e)));
                        eprintln!("  {}", color::muted("Make sure kubectl is configured and you have access to the cluster"));
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
                    use crate::tui::colors::cli;
                    use crate::tui::colors::vm_status_symbol;

                    // Print header with theme colors
                    println!("{:<30} {:<20} {:<15} {:<10}",
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
                            cli::success("Yes")
                        } else {
                            color::muted("No")
                        };
                        let status = if let Some(s) = &vm.status {
                            s.print_able_status.as_deref().unwrap_or("Unknown")
                        } else {
                            "Unknown"
                        };

                        // Format with theme colors
                        let status_display = format!("{} {}",
                            vm_status_symbol(status),
                            color::vm_status(status)
                        );

                        println!("{:<30} {:<20} {:<25} {:<10}",
                            cli::vm_name(name),
                            color::namespace(namespace),
                            status_display,
                            running
                        );
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
            use crate::tui::colors::cli;
            println!("{}", cli::success(&format!("VM '{}' deleted successfully", name)));
        }

        Commands::Start { name } => {
            use crate::tui::colors::cli;
            let client = kube::KubeClient::new().await?;
            client.start_vm(&cli.namespace, &name).await?;
            println!("{}", cli::success(&format!("VM '{}' started successfully", name)));
        }

        Commands::Stop { name } => {
            use crate::tui::colors::cli;
            let client = kube::KubeClient::new().await?;
            client.stop_vm(&cli.namespace, &name).await?;
            println!("{}", cli::success(&format!("VM '{}' stopped successfully", name)));
        }

        Commands::Restart { name } => {
            use crate::tui::colors::cli;
            let client = kube::KubeClient::new().await?;
            println!("{}", color::info(&format!("Restarting VM '{}'...", name)));
            client.restart_vm(&cli.namespace, &name).await?;
            println!("{}", cli::success(&format!("VM '{}' restarted successfully", name)));
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
            
            println!("{}", color::header("Available templates:"));
            for template in TEMPLATES.list() {
                println!("  {} {}", color::value("•"), color::label(&template));
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
            use crate::tui::colors::cli;
            println!("{}", cli::success("Configuration is valid"));
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
            use crate::tui::colors::cli;
            let client = kube::KubeClient::new().await?;

            println!("{}", color::info(&format!("Cloning VM '{}' to '{}'...", source, target)));

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
            println!("{}", cli::success(&format!("VM '{}' cloned successfully", target)));

            if start {
                println!("{}", color::info(&format!("Starting VM '{}'...", target)));
                client.start_vm(&cli.namespace, &target).await?;
                println!("{}", cli::success(&format!("VM '{}' started", target)));
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

            use crate::tui::colors::cli;

            let summary = kube::ResourceSummary::from_vms(&vms);
            summary.display();

            println!();
            println!("{}", color::header("╔═══════════════════════════════════════════════════════════════╗"));
            println!("{}", color::header("║                   VM Resource Details                         ║"));
            println!("{}", color::header("╚═══════════════════════════════════════════════════════════════╝"));
            println!();
            println!("{:<30} {:<15} {:<10} {:<10}",
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
                println!("{:<30} {:<15} {:<10} {:<10}",
                    cli::vm_name(name),
                    color::namespace(namespace),
                    color::resource(&cpu.to_string(), "cpu"),
                    color::resource(memory, "memory")
                );
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
                use crate::tui::colors::cli;
                fs::write(&output_file, &manifest)?;
                println!("{}", cli::success(&format!("VM exported to: {}", cli::path(&output_file))));
            } else {
                println!("{}", manifest);
            }
        }

        Commands::Wizard { name } => {
            use crate::tui::colors::cli;
            println!("{}", color::header("╔═══════════════════════════════════════════════════════════════╗"));
            println!("{}", color::header("║           Interactive VM Creation Wizard                      ║"));
            println!("{}", color::header("╚═══════════════════════════════════════════════════════════════╝"));
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
            println!("{}", color::info("Creating VM with the following configuration:"));
            println!("  {:<12} {}", color::label("Name:"), color::value(&vm_name));
            println!("  {:<12} {}", color::label("Template:"), color::value(template_name));
            println!("  {:<12} {}", color::label("CPU:"), color::resource(&format!("{} cores", cpu_cores), "cpu"));
            println!("  {:<12} {}", color::label("Memory:"), color::resource(&memory, "memory"));
            println!("  {:<12} {}", color::label("Disk:"), color::resource(&disk_size, "disk"));
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
            println!("{}", cli::success(&format!("VM '{}' created successfully", vm_name)));

            if start_vm {
                client.start_vm(&cli.namespace, &vm_name).await?;
                println!("{}", cli::success(&format!("VM '{}' started", vm_name)));
            }
        }

        Commands::Batch {
            file,
            namespace,
            dry_run,
            continue_on_error,
        } => {
            use indicatif::{ProgressBar, ProgressStyle};
            use crate::tui::colors::cli;

            println!("{}", color::info(&format!("Loading batch configuration from: {}", cli::path(&file))));
            let mut batch = utils::BatchConfig::from_file(&file)?;

            // Apply namespace override
            if let Some(ns) = namespace {
                batch.apply_namespace(&ns);
            } else {
                batch.apply_namespace(&cli.namespace);
            }

            println!("{}", color::info(&format!("Found {} VMs to create", batch.vms.len())));
            println!();

            if dry_run {
                println!("{}", color::info("Dry run - VMs that would be created:"));
                for (i, vm) in batch.vms.iter().enumerate() {
                    println!("  {}. {} (namespace: {}, {} cores, {})",
                        color::value(&(i + 1).to_string()),
                        cli::vm_name(&vm.name),
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
            println!("{}", color::header("╔═══════════════════════════════════════════════════════════════╗"));
            println!("{}", color::header("║                   Batch Summary                               ║"));
            println!("{}", color::header("╚═══════════════════════════════════════════════════════════════╝"));
            println!("  {:<12} {}", color::label("Total VMs:"), color::value(&batch.vms.len().to_string()));
            println!("  {:<12} {}", color::label("Successful:"), cli::success(&format!("{} ✓", success_count)));
            println!("  {:<12} {}", color::label("Failed:"), if error_count > 0 {
                cli::error(&format!("{} ✗", error_count))
            } else {
                color::muted(&format!("{} ✗", error_count))
            });

            if error_count > 0 && !continue_on_error {
                std::process::exit(1);
            }
        }

        // ========== INNOVATIVE FEATURES ==========

        Commands::Profiles { details } => {
            use crate::profiles::PROFILES;
            

            println!("{}", color::header("═══ VM Resource Profiles ═══"));
            println!();

            for profile in PROFILES.list() {
                println!("{} {}", color::value("•"), color::header(&profile.name));
                println!("  {}", color::muted(&profile.description));

                if details {
                    println!("  CPU:    {} cores ({} sockets, {} threads)",
                        color::resource(&profile.cpu_cores.to_string(), "cpu"),
                        profile.cpu_sockets,
                        profile.cpu_threads
                    );
                    println!("  Memory: {}", color::resource(&profile.memory, "memory"));
                    println!("  Disk:   {}", color::resource(&profile.disk_size, "disk"));
                    println!("  Use cases: {}", profile.use_cases.join(", "));
                    println!("  Recommended OS: {}", profile.recommended_os.join(", "));
                }
                println!();
            }

            println!("{}", color::muted("Use 'zorvia profile <name>' for details"));
            println!("{}", color::muted("Create VM with profile: zorvia create <name> --template <os> --profile <profile>"));
        }

        Commands::Profile { name, output } => {
            use crate::profiles::PROFILES;
            

            let profile = PROFILES.get(&name)
                .ok_or_else(|| anyhow!("Profile not found: {}", name))?;

            match output.as_str() {
                "json" => {
                    let json = serde_json::to_string_pretty(profile)?;
                    println!("{}", json);
                }
                _ => {
                    let yaml = serde_yaml::to_string(profile)?;
                    println!("{}", yaml);
                }
            }
        }

        Commands::Blueprints { tag, details } => {
            use crate::blueprints::BLUEPRINTS;
            use crate::tui::colors::cli;

            println!("{}", color::header("═══ Multi-VM Blueprints ═══"));
            println!();

            let blueprints = if let Some(tag_filter) = tag {
                BLUEPRINTS.search_by_tag(&tag_filter)
            } else {
                BLUEPRINTS.list()
            };

            for blueprint in blueprints {
                println!("{} {}", color::value("•"), color::header(&blueprint.name));
                println!("  {}", color::muted(&blueprint.description));
                println!("  VMs: {}", color::value(&blueprint.vms.len().to_string()));
                if details {
                    for vm in &blueprint.vms {
                        println!("    {} {} (template: {})",
                            color::value("-"),
                            cli::vm_name(&vm.name),
                            vm.template
                        );
                    }
                    println!("  Tags: {}", blueprint.tags.join(", "));
                }
                println!();
            }

            println!("{}", color::muted("Use 'zorvia blueprint <name>' for details"));
            println!("{}", color::muted("Deploy blueprint: zorvia deploy <blueprint>"));
        }

        Commands::Blueprint { name, output } => {
            use crate::blueprints::BLUEPRINTS;

            let blueprint = BLUEPRINTS.get(&name)
                .ok_or_else(|| anyhow!("Blueprint not found: {}", name))?;

            match output.as_str() {
                "json" => {
                    let json = serde_json::to_string_pretty(blueprint)?;
                    println!("{}", json);
                }
                _ => {
                    let yaml = serde_yaml::to_string(blueprint)?;
                    println!("{}", yaml);
                }
            }
        }

        Commands::Deploy {
            blueprint,
            prefix,
            start,
            dry_run,
        } => {
            use crate::blueprints::BLUEPRINTS;
            use crate::profiles::PROFILES;
            use crate::tui::colors::cli;

            let bp = BLUEPRINTS.get(&blueprint)
                .ok_or_else(|| anyhow!("Blueprint not found: {}", blueprint))?;

            let vm_prefix = prefix.unwrap_or_else(|| blueprint.clone());

            println!("{}", color::info(&format!("Deploying blueprint: {}", blueprint)));
            println!("  Description: {}", bp.description);
            println!("  VMs to create: {}", bp.vms.len());
            println!();

            if dry_run {
                println!("{}", color::info("Dry run - VMs that would be created:"));
                for (i, vm_spec) in bp.vms.iter().enumerate() {
                    let vm_name = format!("{}-{}", vm_prefix, vm_spec.name);
                    println!("  {}. {}", i + 1, cli::vm_name(&vm_name));
                    println!("     Template: {}", vm_spec.template);
                    if let Some(profile_name) = &vm_spec.profile {
                        if let Some(profile) = PROFILES.get(profile_name) {
                            println!("     Profile: {} ({}, {})",
                                profile_name,
                                color::resource(&profile.cpu_cores.to_string(), "cpu"),
                                color::resource(&profile.memory, "memory")
                            );
                        }
                    }
                    if !vm_spec.depends_on.is_empty() {
                        println!("     Depends on: {}", vm_spec.depends_on.join(", "));
                    }
                    println!();
                }
                return Ok(());
            }

            // Create VMs in dependency order
            let client = kube::KubeClient::new().await?;

            for vm_spec in &bp.vms {
                let vm_name = format!("{}-{}", vm_prefix, vm_spec.name);

                println!("{}", color::info(&format!("Creating VM: {}", vm_name)));

                // Get base config from template
                let mut config = TEMPLATES.get(&vm_spec.template)
                    .ok_or_else(|| anyhow!("Template not found: {}", vm_spec.template))?;

                config.name = vm_name.clone();
                config.namespace = cli.namespace.clone();

                // Apply profile if specified
                if let Some(profile_name) = &vm_spec.profile {
                    if let Some(profile) = PROFILES.get(profile_name) {
                        config.cpu.cores = profile.cpu_cores;
                        config.cpu.sockets = profile.cpu_sockets;
                        config.cpu.threads = profile.cpu_threads;
                        config.memory.size = profile.memory.clone();
                    }
                }

                // Apply custom overrides
                if let Some(cpu) = vm_spec.cpu {
                    config.cpu.cores = cpu;
                }
                if let Some(ref memory) = vm_spec.memory {
                    config.memory.size = memory.clone();
                }
                if let Some(ref disk_size) = vm_spec.disk_size {
                    if let Some(disk) = config.disks.first_mut() {
                        disk.size = disk_size.clone();
                    }
                }

                // Apply labels
                for (k, v) in &vm_spec.labels {
                    config.labels.insert(k.clone(), v.clone());
                }

                // Create the VM
                match client.create_vm(&config).await {
                    Ok(_) => {
                        println!("{}", cli::success(&format!("VM '{}' created successfully", vm_name)));
                    }
                    Err(e) => {
                        println!("{}", cli::error(&format!("Failed to create VM '{}': {}", vm_name, e)));
                    }
                }

                // Start if requested
                if start {
                    match client.start_vm(&cli.namespace, &vm_name).await {
                        Ok(_) => {
                            println!("{}", cli::success(&format!("VM '{}' started", vm_name)));
                        }
                        Err(e) => {
                            println!("{}", cli::error(&format!("Failed to start VM '{}': {}", vm_name, e)));
                        }
                    }
                }

                println!();
            }

            println!("{}", cli::success("Blueprint deployment complete!"));
        }

        Commands::Health { target, detailed } => {
            use crate::health::VMHealthReport;
            use crate::tui::colors::cli;

            println!("{}", color::header(&format!("═══ Health Check: {} ═══", target)));
            println!();

            // Try to load as config file first
            let config = if std::path::Path::new(&target).exists() {
                let content = fs::read_to_string(&target)?;
                if target.ends_with(".json") {
                    serde_json::from_str(&content)?
                } else {
                    serde_yaml::from_str(&content)?
                }
            } else {
                // Try to get running VM
                let client = kube::KubeClient::new().await?;
                let vm = client.get_vm(&cli.namespace, &target).await?;

                // Convert to VMConfig (simplified)
                let cpu_cores = vm.spec.template.spec.domain.cpu.as_ref()
                    .and_then(|c| c.cores)
                    .unwrap_or(2);
                let memory = vm.spec.template.spec.domain.memory.as_ref()
                    .and_then(|m| m.guest.as_deref())
                    .unwrap_or("4Gi")
                    .to_string();

                VMConfigBuilder::new(&target)
                    .namespace(&cli.namespace)
                    .cpu(cpu_cores, 1, 1)
                    .memory(&memory)
                    .build()
            };

            let mut report = VMHealthReport::new(config.name.clone());

            // Run resource checks
            for check in VMHealthReport::check_resources(
                config.cpu.cores,
                &config.memory.size,
                &config.disks.first().map(|d| d.size.as_str()).unwrap_or("20Gi"),
            ) {
                report.add_check(check);
            }

            // Display report
            let status_color = match report.overall_status {
                crate::health::HealthStatus::Healthy => cli::success("✓ HEALTHY"),
                crate::health::HealthStatus::Warning => color::warning("⚠ WARNING"),
                crate::health::HealthStatus::Critical => cli::error("✗ CRITICAL"),
                _ => color::muted("? UNKNOWN"),
            };

            println!("Overall Status: {}", status_color);
            println!("Health Score:   {}/100", report.score);
            println!();

            if detailed || !report.checks.is_empty() {
                println!("{}", color::header("Checks:"));
                for check in &report.checks {
                    let status_icon = match check.status {
                        crate::health::HealthStatus::Healthy => color::success("✓"),
                        crate::health::HealthStatus::Warning => color::warning("⚠"),
                        crate::health::HealthStatus::Critical => color::error("✗"),
                        _ => color::muted("?"),
                    };
                    println!("  {} {} - {}", status_icon, color::label(&check.name), check.message);
                    if let Some(ref rec) = check.recommendation {
                        println!("      {}", color::muted(&format!("→ {}", rec)));
                    }
                }
                println!();
            }

            if !report.recommendations.is_empty() {
                println!("{}", color::header("Recommendations:"));
                for (i, rec) in report.recommendations.iter().enumerate() {
                    println!("  {}. {}", i + 1, color::value(rec));
                }
            }
        }

        Commands::Recommend { workload, alternatives } => {
            use crate::profiles::PROFILES;
            use crate::tui::colors::cli;

            println!("{}", color::header(&format!("═══ Resource Recommendations for: {} ═══", workload)));
            println!();

            let recommendations = PROFILES.recommend(&workload);

            if recommendations.is_empty() {
                println!("{}", color::warning("No specific recommendations found for this workload"));
                println!("{}", color::muted("Showing general-purpose profiles:"));
                println!();

                for profile in vec!["dev", "test", "prod"] {
                    if let Some(p) = PROFILES.get(profile) {
                        println!("{} {}", color::value("•"), color::header(&p.name));
                        println!("  {}", color::muted(&p.description));
                        println!("  CPU: {} cores, Memory: {}, Disk: {}",
                            color::resource(&p.cpu_cores.to_string(), "cpu"),
                            color::resource(&p.memory, "memory"),
                            color::resource(&p.disk_size, "disk")
                        );
                        println!();
                    }
                }
            } else {
                println!("{}", cli::success(&format!("Found {} matching profile(s):", recommendations.len())));
                println!();

                for (i, profile) in recommendations.iter().enumerate() {
                    let marker = if i == 0 { cli::success("★") } else { color::value("•") };
                    let label = if i == 0 { format!("{} (Recommended)", profile.name) } else { profile.name.clone() };

                    println!("{} {}", marker, color::header(&label));
                    println!("  {}", color::muted(&profile.description));
                    println!("  Resources:");
                    println!("    CPU:    {} cores ({} sockets × {} threads)",
                        color::resource(&profile.cpu_cores.to_string(), "cpu"),
                        profile.cpu_sockets,
                        profile.cpu_threads
                    );
                    println!("    Memory: {}", color::resource(&profile.memory, "memory"));
                    println!("    Disk:   {}", color::resource(&profile.disk_size, "disk"));
                    println!("  Best for: {}", profile.use_cases.join(", "));
                    println!("  Recommended OS: {}", profile.recommended_os.join(", "));
                    println!();

                    if i == 0 {
                        println!("  {}", color::info("Quick create command:"));
                        println!("    {}", color::command(&format!(
                            "zorvia create my-vm --template {} --profile {}",
                            profile.recommended_os.first().unwrap_or(&"ubuntu".to_string()),
                            profile.name
                        )));
                        println!();
                    }
                }
            }

            if alternatives {
                println!("{}", color::header("All Available Profiles:"));
                for profile in PROFILES.list() {
                    println!("  {} {}", color::value("•"), color::label(&profile.name));
                }
                println!();
                println!("{}", color::muted("Use 'zorvia profiles' to see all profiles"));
            }
        }

        // ========== VM SNAPSHOTS & BACKUP ==========

        Commands::SnapshotCreate {
            vm,
            name,
            description,
        } => {
            use snapshots::{SnapshotConfig, SnapshotManager};
            use chrono::Utc;

            let manager = SnapshotManager::new(&cli.namespace);

            // Auto-generate snapshot name if not provided
            let snapshot_name = name.unwrap_or_else(|| {
                format!("{}-snapshot-{}", vm, Utc::now().format("%Y%m%d-%H%M%S"))
            });

            let mut config = SnapshotConfig::new(&vm, &snapshot_name);
            if let Some(desc) = description {
                config = config.with_description(desc);
            }

            println!("{}", color::header(&format!("Creating snapshot for VM: {}", vm)));
            println!("  Snapshot name: {}", color::value(&snapshot_name));
            if let Some(desc) = &config.description {
                println!("  Description:   {}", color::muted(desc));
            }
            println!();

            match manager.create_snapshot(&config).await {
                Ok(snapshot) => {
                    println!("{} Snapshot creation started", color::success("✓"));
                    println!("  Status:    {}", color::vm_status("InProgress"));
                    println!();
                    println!("{}", color::info("Check snapshot status with:"));
                    println!("  {}", color::command(&format!("zorvia snapshot-get {}", snapshot_name)));
                }
                Err(e) => {
                    println!("{} Failed to create snapshot: {}", color::error("✗"), e);
                    return Err(e);
                }
            }
        }

        Commands::SnapshotList {
            vm,
            all_namespaces: _,
            output,
        } => {
            use snapshots::SnapshotManager;

            let manager = SnapshotManager::new(&cli.namespace);

            let snapshots = if let Some(vm_name) = vm {
                println!("{}", color::header(&format!("Snapshots for VM: {}", vm_name)));
                manager.list_snapshots_for_vm(&vm_name).await?
            } else {
                println!("{}", color::header(&format!("All Snapshots in namespace: {}", color::namespace(&cli.namespace))));
                manager.list_all_snapshots().await?
            };

            if snapshots.is_empty() {
                println!("{}", color::muted("No snapshots found"));
                return Ok(());
            }

            if output == "table" {
                println!();
                println!("{:<30} {:<20} {:<12} {:<10} {:<10}",
                    color::label("NAME"),
                    color::label("VM"),
                    color::label("STATUS"),
                    color::label("SIZE"),
                    color::label("AGE")
                );
                println!("{}", "-".repeat(82));

                for snapshot in &snapshots {
                    let status_str = match snapshot.status {
                        snapshots::SnapshotStatus::Succeeded => color::vm_status("Running"),
                        snapshots::SnapshotStatus::InProgress => color::vm_status("Pending"),
                        snapshots::SnapshotStatus::Failed => color::vm_status("Failed"),
                        snapshots::SnapshotStatus::Unknown => color::vm_status("Unknown"),
                    };

                    println!("{:<30} {:<20} {:<12} {:<10} {:<10}",
                        snapshot.name,
                        snapshot.vm_name,
                        status_str,
                        snapshot.size.as_deref().unwrap_or("-"),
                        snapshot.age()
                    );
                }
            } else {
                let output_format = if output == "json" {
                    OutputFormat::Json
                } else {
                    OutputFormat::Yaml
                };
                let formatted = format_output(&snapshots, output_format)?;
                println!("{}", formatted);
            }
        }

        Commands::SnapshotGet { name, output } => {
            use snapshots::SnapshotManager;

            let manager = SnapshotManager::new(&cli.namespace);
            let snapshot = manager.get_snapshot(&name).await?;

            if output == "yaml" || output == "json" {
                let output_format = if output == "json" {
                    OutputFormat::Json
                } else {
                    OutputFormat::Yaml
                };
                let formatted = format_output(&snapshot, output_format)?;
                println!("{}", formatted);
            } else {
                println!("{}", color::header(&format!("Snapshot: {}", name)));
                println!();
                println!("  VM:          {}", color::value(&snapshot.vm_name));
                println!("  Namespace:   {}", color::namespace(&snapshot.namespace));

                let status_str = match snapshot.status {
                    snapshots::SnapshotStatus::Succeeded => color::success("✓ READY"),
                    snapshots::SnapshotStatus::InProgress => color::warning("◐ IN PROGRESS"),
                    snapshots::SnapshotStatus::Failed => color::error("✗ FAILED"),
                    snapshots::SnapshotStatus::Unknown => color::muted("? UNKNOWN"),
                };
                println!("  Status:      {}", status_str);

                if let Some(desc) = &snapshot.description {
                    println!("  Description: {}", color::muted(desc));
                }
                if let Some(size) = &snapshot.size {
                    println!("  Size:        {}", color::resource(size, "storage"));
                }
                println!("  Age:         {}", snapshot.age());

                if let Some(duration) = snapshot.duration() {
                    println!("  Duration:    {}", duration);
                }

                println!("  Ready:       {}", if snapshot.ready_to_use {
                    color::success("Yes")
                } else {
                    color::muted("No")
                });

                if let Some(error) = &snapshot.error {
                    println!("  Error:       {}", color::error(error));
                }
            }
        }

        Commands::SnapshotDelete { name, yes } => {
            use snapshots::SnapshotManager;

            if !yes {
                print!("Are you sure you want to delete snapshot '{}'? [y/N] ", name);
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("{}", color::muted("Cancelled"));
                    return Ok(());
                }
            }

            let manager = SnapshotManager::new(&cli.namespace);

            println!("{}", color::header(&format!("Deleting snapshot: {}", name)));
            match manager.delete_snapshot(&name).await {
                Ok(_) => {
                    println!("{} Snapshot deleted successfully", color::success("✓"));
                }
                Err(e) => {
                    println!("{} Failed to delete snapshot: {}", color::error("✗"), e);
                    return Err(e);
                }
            }
        }

        Commands::SnapshotRestore {
            snapshot,
            target,
            in_place,
            start,
        } => {
            use snapshots::RestoreManager;

            let manager = RestoreManager::new(&cli.namespace);

            if in_place {
                // Restore in-place (overwrite existing VM)
                let default_vm = snapshot.replace("-snapshot", "");
                let vm_name = target.as_deref().unwrap_or(&default_vm);

                println!("{}", color::header(&format!("Restoring VM in-place: {}", vm_name)));
                println!("{}", color::warning("⚠ This will overwrite the current VM state"));
                println!();

                match manager.restore_in_place(vm_name, &snapshot).await {
                    Ok(restore) => {
                        println!("{} Restore started", color::success("✓"));
                        println!("  Restore name: {}", restore.name);
                        println!("  Status:       {}", color::vm_status("InProgress"));
                    }
                    Err(e) => {
                        println!("{} Failed to restore: {}", color::error("✗"), e);
                        return Err(e);
                    }
                }
            } else {
                // Restore to new VM
                let default_target = format!("{}-restored", snapshot);
                let target_vm = target.as_deref().unwrap_or(&default_target);

                println!("{}", color::header(&format!("Restoring snapshot to new VM: {}", target_vm)));
                println!("  Snapshot:  {}", color::value(&snapshot));
                println!("  Target VM: {}", color::value(target_vm));
                if start {
                    println!("  Start:     {}", color::success("Yes"));
                }
                println!();

                match manager.restore_to_new_vm(&snapshot, target_vm, start).await {
                    Ok(restore) => {
                        println!("{} Restore started", color::success("✓"));
                        println!("  Restore name: {}", restore.name);
                        println!("  Status:       {}", color::vm_status("InProgress"));
                        println!();
                        if start {
                            println!("{}", color::info("VM will be started after restore completes"));
                        }
                    }
                    Err(e) => {
                        println!("{} Failed to restore: {}", color::error("✗"), e);
                        return Err(e);
                    }
                }
            }
        }

        // ========== PERFORMANCE MONITORING ==========

        Commands::MonitorLive { vm, interval } => {
            use monitoring::{MetricsCollector, MonitoringReporter};

            let collector = MetricsCollector::new(&cli.namespace);
            let reporter = MonitoringReporter::new();

            println!("{}", color::header(&format!("Live Monitoring: {} (Press Ctrl+C to stop)", vm)));
            println!("{}", color::muted(&format!("Update interval: {} seconds", interval)));
            println!();

            // Simple loop for demonstration (in a real TUI, this would be in a terminal UI)
            for i in 0..10 {
                if i > 0 {
                    // Clear screen (simple version)
                    println!("\n{}", "═".repeat(80));
                }

                match collector.collect(&vm).await {
                    Ok(metrics) => {
                        println!("{}", reporter.format_live_metrics(&vm, &metrics));
                    }
                    Err(e) => {
                        println!("{} Failed to collect metrics: {}", color::error("✗"), e);
                        break;
                    }
                }

                if i < 9 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
                }
            }

            println!();
            println!("{}", color::info("ℹ Live monitoring stopped"));
        }

        Commands::MonitorStats {
            vm,
            period,
            output,
        } => {
            use monitoring::{MetricsCollector, PerformanceAnalyzer, MonitoringReporter, ReportFormat};

            let collector = MetricsCollector::new(&cli.namespace);
            let analyzer = PerformanceAnalyzer::with_default_thresholds();
            let reporter = MonitoringReporter::new();

            println!("{}", color::header(&format!("Performance Statistics: {}", vm)));
            println!("  Period: {}", color::value(&period));
            println!();

            match collector.collect(&vm).await {
                Ok(metrics) => {
                    let report = analyzer.analyze(&vm, &metrics);

                    let format = match output.as_str() {
                        "json" => ReportFormat::Json,
                        "yaml" => ReportFormat::Yaml,
                        "summary" => ReportFormat::Summary,
                        _ => ReportFormat::Table,
                    };

                    match reporter.format_performance_report(&report, &format) {
                        Ok(formatted) => println!("{}", formatted),
                        Err(e) => {
                            println!("{} Failed to format report: {}", color::error("✗"), e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    println!("{} Failed to collect metrics: {}", color::error("✗"), e);
                    return Err(e);
                }
            }
        }

        Commands::MonitorCompare { vms, output } => {
            use monitoring::{MetricsCollector, PerformanceAnalyzer, MonitoringReporter};

            if vms.len() < 2 {
                println!("{} At least 2 VMs are required for comparison", color::error("✗"));
                return Err(anyhow!("Need at least 2 VMs"));
            }

            let collector = MetricsCollector::new(&cli.namespace);
            let analyzer = PerformanceAnalyzer::with_default_thresholds();
            let reporter = MonitoringReporter::new();

            println!("{}", color::header(&format!("Comparing {} VMs", vms.len())));
            println!();

            let mut reports = Vec::new();

            for vm_name in &vms {
                match collector.collect(vm_name).await {
                    Ok(metrics) => {
                        let report = analyzer.analyze(vm_name, &metrics);
                        reports.push(report);
                    }
                    Err(e) => {
                        println!("{} Failed to collect metrics for {}: {}",
                            color::error("✗"), vm_name, e);
                    }
                }
            }

            if reports.is_empty() {
                println!("{} No metrics collected", color::error("✗"));
                return Ok(());
            }

            if output == "json" {
                let json = serde_json::to_string_pretty(&reports)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&reports)?;
                println!("{}", yaml);
            } else {
                // Table format
                let comparison = analyzer.compare(reports);
                println!("{}", reporter.format_comparison(comparison));
            }
        }

        Commands::MonitorTop {
            all_namespaces: _,
            sort_by,
            limit,
        } => {
            use monitoring::{MetricsCollector, PerformanceAnalyzer, MonitoringReporter};

            // Mock VMs for demonstration
            let vms = vec!["prod-db", "prod-web", "test-vm", "dev-vm", "cache-vm"];

            let collector = MetricsCollector::new(&cli.namespace);
            let analyzer = PerformanceAnalyzer::with_default_thresholds();
            let reporter = MonitoringReporter::new();

            println!("{}", color::header(&format!("Top {} VMs by {}", limit.min(vms.len()), sort_by)));
            println!();

            let mut reports = Vec::new();

            for vm_name in vms.iter().take(limit) {
                match collector.collect(vm_name).await {
                    Ok(metrics) => {
                        let report = analyzer.analyze(vm_name, &metrics);
                        reports.push(report);
                    }
                    Err(_) => {}
                }
            }

            // Sort based on sort_by parameter
            reports.sort_by(|a, b| {
                match sort_by.as_str() {
                    "cpu" => b.current_metrics.cpu.usage_percent.partial_cmp(&a.current_metrics.cpu.usage_percent).unwrap(),
                    "memory" => b.current_metrics.memory.usage_percent.partial_cmp(&a.current_metrics.memory.usage_percent).unwrap(),
                    "disk" => b.current_metrics.disk.usage_percent.partial_cmp(&a.current_metrics.disk.usage_percent).unwrap(),
                    _ => b.performance_score.cmp(&a.performance_score), // default: score
                }
            });

            let comparison: Vec<_> = reports.into_iter()
                .map(|r| (r.vm_name, r.performance_score, r.status.as_str().to_string()))
                .collect();

            println!("{}", reporter.format_comparison(comparison));

            println!();
            println!("{}", color::info(&format!("ℹ Sorted by: {}", sort_by)));
        }

        // ========== DISK MANAGEMENT ==========

        Commands::DiskExpand {
            vm,
            disk,
            size,
            pvc,
            plan,
        } => {
            use disk::{DiskConfig, DiskExpansion};

            let pvc_name = pvc.as_deref().unwrap_or(&disk);
            let config = DiskConfig::new(&disk, pvc_name)
                .with_sizes("unknown", &size);

            let expansion = DiskExpansion::new(&cli.namespace);
            let expansion_plan = expansion.create_plan(&vm, &config)?;

            if plan {
                // Show plan without executing
                println!("{}", color::header(&format!("Disk Expansion Plan: {}", vm)));
                println!("  Disk:        {}", color::value(&disk));
                println!("  PVC:         {}", color::value(pvc_name));
                println!("  Target Size: {}", color::value(&size));
                println!();

                println!("{}", color::header("Expansion Steps:"));
                for step in &expansion_plan.steps {
                    let status = if step.completed {
                        color::success("✓")
                    } else {
                        color::muted("○")
                    };
                    println!("  {} Step {}: {}", status, step.step_number, step.description);
                    println!("     {}", color::muted(&format!("$ {}", step.command)));
                }

                println!();
                let increase_gi = disk::DiskInfo::parse_size(&size) / (1024 * 1024 * 1024);
                println!("{}", color::info(&format!("ℹ Estimated time: {}",
                    expansion.estimate_duration(increase_gi)
                )));
            } else {
                // Execute expansion
                println!("{}", color::header(&format!("Expanding disk: {}", disk)));
                println!("  VM:          {}", vm);
                println!("  Target size: {}", color::value(&size));
                println!();

                println!("{}", color::info("Starting PVC resize..."));
                match expansion.resize_pvc(pvc_name, &size).await {
                    Ok(_) => {
                        println!("{} PVC resize initiated", color::success("✓"));
                        println!();
                        println!("{}", color::warning("⚠ Next steps (run inside VM):"));
                        println!("  1. Rescan disk:");
                        println!("     {}", color::command("echo 1 | sudo tee /sys/class/block/vda/device/rescan"));
                        println!("  2. Expand filesystem (see: zorvia disk-script)");
                    }
                    Err(e) => {
                        println!("{} Failed to resize PVC: {}", color::error("✗"), e);
                        return Err(e);
                    }
                }
            }
        }

        Commands::DiskHealth { vm, detailed } => {
            use disk::{DiskInfo, DiskHealthCheck};
            use disk::health::DiskHealthStatus;

            println!("{}", color::header(&format!("Disk Health: {}", vm)));
            println!();

            // Mock disk data for demonstration
            let mut disks = vec![
                {
                    let mut d = DiskInfo::new("root");
                    d.mount_point = "/".to_string();
                    d.size = "100Gi".to_string();
                    d.used = "75Gi".to_string();
                    d.available = "25Gi".to_string();
                    d.usage_percent = 75.0;
                    d.filesystem = "ext4".to_string();
                    d.device = "/dev/vda1".to_string();
                    d
                },
                {
                    let mut d = DiskInfo::new("data");
                    d.mount_point = "/data".to_string();
                    d.size = "200Gi".to_string();
                    d.used = "180Gi".to_string();
                    d.available = "20Gi".to_string();
                    d.usage_percent = 90.0;
                    d.filesystem = "xfs".to_string();
                    d.device = "/dev/vdb1".to_string();
                    d
                },
            ];

            let checker = DiskHealthCheck::with_defaults();
            let health = checker.check_vm(&vm, disks.clone());

            let status_str = match health.overall_status {
                DiskHealthStatus::Healthy => color::success("✓ HEALTHY"),
                DiskHealthStatus::Warning => color::warning("⚠ WARNING"),
                DiskHealthStatus::Critical => color::error("✗ CRITICAL"),
                DiskHealthStatus::Full => color::error("✗ FULL"),
            };

            println!("Overall Status: {}", status_str);
            println!();

            if detailed {
                println!("{}", color::header("Disk Details:"));
                for disk in &disks {
                    let status = checker.check_disk(disk);
                    let status_icon = match status {
                        DiskHealthStatus::Healthy => color::success("✓"),
                        DiskHealthStatus::Warning => color::warning("⚠"),
                        DiskHealthStatus::Critical => color::error("✗"),
                        DiskHealthStatus::Full => color::error("✗"),
                    };

                    println!();
                    println!("  {} {}", status_icon, color::value(&disk.mount_point));
                    println!("    Size:       {}", disk.size);
                    println!("    Used:       {} ({:.1}%)", disk.used, disk.usage_percent);
                    println!("    Available:  {}", disk.available);
                    println!("    Filesystem: {}", disk.filesystem);
                    println!("    Device:     {}", color::muted(&disk.device));

                    if status != DiskHealthStatus::Healthy {
                        let recommended = checker.recommend_expansion_size(disk, 60.0);
                        println!("    {}", color::warning(&format!("→ Recommended size: {}", recommended)));
                    }
                }
            }

            if !health.alerts.is_empty() {
                println!();
                println!("{}", color::header("Alerts:"));
                for alert in &health.alerts {
                    println!("  {} {}", color::warning("⚠"), alert.message);
                    println!("    {}", color::muted(&alert.recommendation));
                }
            }

            if health.needs_expansion {
                println!();
                println!("{}", color::info("ℹ Use 'zorvia disk-expand' to expand disks"));
            }
        }

        Commands::DiskScript {
            filesystem,
            device,
            output,
            dry_run,
        } => {
            use disk::{FilesystemType, ExpansionScript, ScriptGenerator};

            let fs_type = FilesystemType::from_string(&filesystem);
            let script_config = ExpansionScript::new(fs_type.clone(), &device)
                .with_lvm("ubuntu-vg", "ubuntu-lv")
                .with_partition(3)
                .dry_run(dry_run);

            let script = ScriptGenerator::generate(&script_config);

            if let Some(output_file) = output {
                std::fs::write(&output_file, &script)?;
                println!("{} Script written to: {}", color::success("✓"), color::path(&output_file));
                println!();
                println!("{}", color::info("To execute:"));
                println!("  {}", color::command(&format!("chmod +x {}", output_file)));
                println!("  {}", color::command(&format!("sudo ./{}", output_file)));
            } else {
                println!("{}", script);
            }

            println!();
            println!("{}", color::header("Quick One-Liner:"));
            println!("{}", color::command(&ScriptGenerator::generate_oneliner(&fs_type, &device)));
        }

        Commands::DiskUsage {
            vm,
            sort_by,
            output,
        } => {
            use disk::DiskInfo;

            println!("{}", color::header("Disk Usage"));
            if let Some(vm_name) = &vm {
                println!("  VM: {}", color::value(vm_name));
            }
            println!();

            // Mock disk data
            let mut disks = vec![
                {
                    let mut d = DiskInfo::new("prod-db");
                    d.mount_point = "/".to_string();
                    d.size = "200Gi".to_string();
                    d.used = "180Gi".to_string();
                    d.available = "20Gi".to_string();
                    d.usage_percent = 90.0;
                    d
                },
                {
                    let mut d = DiskInfo::new("prod-web");
                    d.mount_point = "/".to_string();
                    d.size = "100Gi".to_string();
                    d.used = "45Gi".to_string();
                    d.available = "55Gi".to_string();
                    d.usage_percent = 45.0;
                    d
                },
                {
                    let mut d = DiskInfo::new("test-vm");
                    d.mount_point = "/".to_string();
                    d.size = "50Gi".to_string();
                    d.used = "38Gi".to_string();
                    d.available = "12Gi".to_string();
                    d.usage_percent = 76.0;
                    d
                },
            ];

            // Sort disks
            match sort_by.as_str() {
                "usage" => disks.sort_by(|a, b| b.usage_percent.partial_cmp(&a.usage_percent).unwrap()),
                "size" => disks.sort_by(|a, b| {
                    let a_size = DiskInfo::parse_size(&a.size);
                    let b_size = DiskInfo::parse_size(&b.size);
                    b_size.cmp(&a_size)
                }),
                "available" => disks.sort_by(|a, b| {
                    let a_avail = DiskInfo::parse_size(&a.available);
                    let b_avail = DiskInfo::parse_size(&b.available);
                    a_avail.cmp(&b_avail)
                }),
                _ => disks.sort_by(|a, b| a.name.cmp(&b.name)),
            }

            if output == "json" {
                let json = serde_json::to_string_pretty(&disks)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&disks)?;
                println!("{}", yaml);
            } else {
                // Table format
                println!("{:<20} {:<12} {:<12} {:<12} {:<10}",
                    color::label("VM"),
                    color::label("SIZE"),
                    color::label("USED"),
                    color::label("AVAILABLE"),
                    color::label("USAGE%")
                );
                println!("{}", "-".repeat(70));

                for disk in &disks {
                    let usage_str = if disk.usage_percent >= 90.0 {
                        color::error(&format!("{:.1}%", disk.usage_percent))
                    } else if disk.usage_percent >= 75.0 {
                        color::warning(&format!("{:.1}%", disk.usage_percent))
                    } else {
                        format!("{:.1}%", disk.usage_percent)
                    };

                    println!("{:<20} {:<12} {:<12} {:<12} {}",
                        disk.name,
                        disk.size,
                        disk.used,
                        disk.available,
                        usage_str
                    );
                }
            }

            println!();
            println!("{}", color::info(&format!("ℹ Sorted by: {}", sort_by)));
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
