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
                                use crate::tui::colors::cli;
                                println!("{}", cli::success(&format!("VM '{}' created successfully", name)));
                                println!("  Namespace: {}", cli::namespace(&config.namespace));
                                println!("  Status: {} (use '{}' to start)",
                                    cli::vm_status("Stopped"),
                                    cli::command(&format!("zorvia start {}", name))
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
                        eprintln!("  {}", cli::muted("Make sure kubectl is configured and you have access to the cluster"));
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
                        cli::header("NAME"),
                        cli::header("NAMESPACE"),
                        cli::header("STATUS"),
                        cli::header("RUNNING")
                    );
                    println!("{}", cli::muted(&"-".repeat(75)));

                    for vm in vms {
                        let name = vm.metadata.name.as_deref().unwrap_or("N/A");
                        let namespace = vm.metadata.namespace.as_deref().unwrap_or("N/A");
                        let running = if vm.spec.running.unwrap_or(false) {
                            cli::success("Yes")
                        } else {
                            cli::muted("No")
                        };
                        let status = if let Some(s) = &vm.status {
                            s.print_able_status.as_deref().unwrap_or("Unknown")
                        } else {
                            "Unknown"
                        };

                        // Format with theme colors
                        let status_display = format!("{} {}",
                            vm_status_symbol(status),
                            cli::vm_status(status)
                        );

                        println!("{:<30} {:<20} {:<25} {:<10}",
                            cli::vm_name(name),
                            cli::namespace(namespace),
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
            println!("{}", cli::info(&format!("Restarting VM '{}'...", name)));
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
            use crate::tui::colors::cli;
            println!("{}", cli::header("Available templates:"));
            for template in TEMPLATES.list() {
                println!("  {} {}", cli::value("•"), cli::label(&template));
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

            println!("{}", cli::info(&format!("Cloning VM '{}' to '{}'...", source, target)));

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
                println!("{}", cli::info(&format!("Starting VM '{}'...", target)));
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
            println!("{}", cli::header("╔═══════════════════════════════════════════════════════════════╗"));
            println!("{}", cli::header("║                   VM Resource Details                         ║"));
            println!("{}", cli::header("╚═══════════════════════════════════════════════════════════════╝"));
            println!();
            println!("{:<30} {:<15} {:<10} {:<10}",
                cli::header("NAME"),
                cli::header("NAMESPACE"),
                cli::header("CPU"),
                cli::header("MEMORY")
            );
            println!("{}", cli::muted(&"-".repeat(70)));

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
                    cli::namespace(namespace),
                    cli::resource(&cpu.to_string(), "cpu"),
                    cli::resource(memory, "memory")
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
            println!("{}", cli::header("╔═══════════════════════════════════════════════════════════════╗"));
            println!("{}", cli::header("║           Interactive VM Creation Wizard                      ║"));
            println!("{}", cli::header("╚═══════════════════════════════════════════════════════════════╝"));
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
            println!("{}", cli::info("Creating VM with the following configuration:"));
            println!("  {:<12} {}", cli::label("Name:"), cli::value(&vm_name));
            println!("  {:<12} {}", cli::label("Template:"), cli::value(template_name));
            println!("  {:<12} {}", cli::label("CPU:"), cli::resource(&format!("{} cores", cpu_cores), "cpu"));
            println!("  {:<12} {}", cli::label("Memory:"), cli::resource(&memory, "memory"));
            println!("  {:<12} {}", cli::label("Disk:"), cli::resource(&disk_size, "disk"));
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

            println!("{}", cli::info(&format!("Loading batch configuration from: {}", cli::path(&file))));
            let mut batch = utils::BatchConfig::from_file(&file)?;

            // Apply namespace override
            if let Some(ns) = namespace {
                batch.apply_namespace(&ns);
            } else {
                batch.apply_namespace(&cli.namespace);
            }

            println!("{}", cli::info(&format!("Found {} VMs to create", batch.vms.len())));
            println!();

            if dry_run {
                println!("{}", cli::info("Dry run - VMs that would be created:"));
                for (i, vm) in batch.vms.iter().enumerate() {
                    println!("  {}. {} (namespace: {}, {} cores, {})",
                        cli::value(&(i + 1).to_string()),
                        cli::vm_name(&vm.name),
                        cli::namespace(&vm.namespace),
                        cli::resource(&vm.cpu.cores.to_string(), "cpu"),
                        cli::resource(&vm.memory.size, "memory")
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
            println!("{}", cli::header("╔═══════════════════════════════════════════════════════════════╗"));
            println!("{}", cli::header("║                   Batch Summary                               ║"));
            println!("{}", cli::header("╚═══════════════════════════════════════════════════════════════╝"));
            println!("  {:<12} {}", cli::label("Total VMs:"), cli::value(&batch.vms.len().to_string()));
            println!("  {:<12} {}", cli::label("Successful:"), cli::success(&format!("{} ✓", success_count)));
            println!("  {:<12} {}", cli::label("Failed:"), if error_count > 0 {
                cli::error(&format!("{} ✗", error_count))
            } else {
                cli::muted(&format!("{} ✗", error_count))
            });

            if error_count > 0 && !continue_on_error {
                std::process::exit(1);
            }
        }

        // ========== INNOVATIVE FEATURES ==========

        Commands::Profiles { details } => {
            use crate::profiles::PROFILES;
            use crate::tui::colors::cli;

            println!("{}", cli::header("═══ VM Resource Profiles ═══"));
            println!();

            for profile in PROFILES.list() {
                println!("{} {}", cli::value("•"), cli::header(&profile.name));
                println!("  {}", cli::muted(&profile.description));

                if details {
                    println!("  CPU:    {} cores ({} sockets, {} threads)",
                        cli::resource(&profile.cpu_cores.to_string(), "cpu"),
                        profile.cpu_sockets,
                        profile.cpu_threads
                    );
                    println!("  Memory: {}", cli::resource(&profile.memory, "memory"));
                    println!("  Disk:   {}", cli::resource(&profile.disk_size, "disk"));
                    println!("  Use cases: {}", profile.use_cases.join(", "));
                    println!("  Recommended OS: {}", profile.recommended_os.join(", "));
                }
                println!();
            }

            println!("{}", cli::muted("Use 'zorvia profile <name>' for details"));
            println!("{}", cli::muted("Create VM with profile: zorvia create <name> --template <os> --profile <profile>"));
        }

        Commands::Profile { name, output } => {
            use crate::profiles::PROFILES;
            use crate::tui::colors::cli;

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

            println!("{}", cli::header("═══ Multi-VM Blueprints ═══"));
            println!();

            let blueprints = if let Some(tag_filter) = tag {
                BLUEPRINTS.search_by_tag(&tag_filter)
            } else {
                BLUEPRINTS.list()
            };

            for blueprint in blueprints {
                println!("{} {}", cli::value("•"), cli::header(&blueprint.name));
                println!("  {}", cli::muted(&blueprint.description));
                println!("  VMs: {}", cli::value(&blueprint.vms.len().to_string()));
                if details {
                    for vm in &blueprint.vms {
                        println!("    {} {} (template: {})",
                            cli::value("-"),
                            cli::vm_name(&vm.name),
                            vm.template
                        );
                    }
                    println!("  Tags: {}", blueprint.tags.join(", "));
                }
                println!();
            }

            println!("{}", cli::muted("Use 'zorvia blueprint <name>' for details"));
            println!("{}", cli::muted("Deploy blueprint: zorvia deploy <blueprint>"));
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

            println!("{}", cli::info(&format!("Deploying blueprint: {}", blueprint)));
            println!("  Description: {}", bp.description);
            println!("  VMs to create: {}", bp.vms.len());
            println!();

            if dry_run {
                println!("{}", cli::info("Dry run - VMs that would be created:"));
                for (i, vm_spec) in bp.vms.iter().enumerate() {
                    let vm_name = format!("{}-{}", vm_prefix, vm_spec.name);
                    println!("  {}. {}", i + 1, cli::vm_name(&vm_name));
                    println!("     Template: {}", vm_spec.template);
                    if let Some(profile_name) = &vm_spec.profile {
                        if let Some(profile) = PROFILES.get(profile_name) {
                            println!("     Profile: {} ({}, {})",
                                profile_name,
                                cli::resource(&profile.cpu_cores.to_string(), "cpu"),
                                cli::resource(&profile.memory, "memory")
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

                println!("{}", cli::info(&format!("Creating VM: {}", vm_name)));

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

            println!("{}", cli::header(&format!("═══ Health Check: {} ═══", target)));
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
                crate::health::HealthStatus::Warning => cli::warning("⚠ WARNING"),
                crate::health::HealthStatus::Critical => cli::error("✗ CRITICAL"),
                _ => cli::muted("? UNKNOWN"),
            };

            println!("Overall Status: {}", status_color);
            println!("Health Score:   {}/100", report.score);
            println!();

            if detailed || !report.checks.is_empty() {
                println!("{}", cli::header("Checks:"));
                for check in &report.checks {
                    let status_icon = match check.status {
                        crate::health::HealthStatus::Healthy => cli::success("✓"),
                        crate::health::HealthStatus::Warning => cli::warning("⚠"),
                        crate::health::HealthStatus::Critical => cli::error("✗"),
                        _ => cli::muted("?"),
                    };
                    println!("  {} {} - {}", status_icon, cli::label(&check.name), check.message);
                    if let Some(ref rec) = check.recommendation {
                        println!("      {}", cli::muted(&format!("→ {}", rec)));
                    }
                }
                println!();
            }

            if !report.recommendations.is_empty() {
                println!("{}", cli::header("Recommendations:"));
                for (i, rec) in report.recommendations.iter().enumerate() {
                    println!("  {}. {}", i + 1, cli::value(rec));
                }
            }
        }

        Commands::Recommend { workload, alternatives } => {
            use crate::profiles::PROFILES;
            use crate::tui::colors::cli;

            println!("{}", cli::header(&format!("═══ Resource Recommendations for: {} ═══", workload)));
            println!();

            let recommendations = PROFILES.recommend(&workload);

            if recommendations.is_empty() {
                println!("{}", cli::warning("No specific recommendations found for this workload"));
                println!("{}", cli::muted("Showing general-purpose profiles:"));
                println!();

                for profile in vec!["dev", "test", "prod"] {
                    if let Some(p) = PROFILES.get(profile) {
                        println!("{} {}", cli::value("•"), cli::header(&p.name));
                        println!("  {}", cli::muted(&p.description));
                        println!("  CPU: {} cores, Memory: {}, Disk: {}",
                            cli::resource(&p.cpu_cores.to_string(), "cpu"),
                            cli::resource(&p.memory, "memory"),
                            cli::resource(&p.disk_size, "disk")
                        );
                        println!();
                    }
                }
            } else {
                println!("{}", cli::success(&format!("Found {} matching profile(s):", recommendations.len())));
                println!();

                for (i, profile) in recommendations.iter().enumerate() {
                    let marker = if i == 0 { cli::success("★") } else { cli::value("•") };
                    let label = if i == 0 { format!("{} (Recommended)", profile.name) } else { profile.name.clone() };

                    println!("{} {}", marker, cli::header(&label));
                    println!("  {}", cli::muted(&profile.description));
                    println!("  Resources:");
                    println!("    CPU:    {} cores ({} sockets × {} threads)",
                        cli::resource(&profile.cpu_cores.to_string(), "cpu"),
                        profile.cpu_sockets,
                        profile.cpu_threads
                    );
                    println!("    Memory: {}", cli::resource(&profile.memory, "memory"));
                    println!("    Disk:   {}", cli::resource(&profile.disk_size, "disk"));
                    println!("  Best for: {}", profile.use_cases.join(", "));
                    println!("  Recommended OS: {}", profile.recommended_os.join(", "));
                    println!();

                    if i == 0 {
                        println!("  {}", cli::info("Quick create command:"));
                        println!("    {}", cli::command(&format!(
                            "zorvia create my-vm --template {} --profile {}",
                            profile.recommended_os.first().unwrap_or(&"ubuntu".to_string()),
                            profile.name
                        )));
                        println!();
                    }
                }
            }

            if alternatives {
                println!("{}", cli::header("All Available Profiles:"));
                for profile in PROFILES.list() {
                    println!("  {} {}", cli::value("•"), cli::label(&profile.name));
                }
                println!();
                println!("{}", cli::muted("Use 'zorvia profiles' to see all profiles"));
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
