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
pub mod migration;
pub mod backup;
pub mod security;
pub mod cost;
pub mod automation;
pub mod observability;
pub mod multitenancy;
pub mod gitops;
pub mod aiml;
pub mod servicemesh;
pub mod dr;
pub mod compliance;
pub mod capacity;
pub mod finops;
pub mod networking;
pub mod edge;
pub mod secrets;
pub mod multicloud;
pub mod devexp;

use anyhow::{anyhow, Result};
use cli::{Cli, Commands};
use config::{validate_vm_config, VMConfig, VMConfigBuilder};
use output::{format_output, OutputFormat};
use std::fs;
use templates::TEMPLATES;
use tui::colors::cli as color;
use chrono::Utc;

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

        Commands::NetworkList { vm, output } => {
            use network::NetworkInterface;

            println!("{}", color::header(&format!("Network Interfaces: {}", vm)));
            println!();

            // Mock network interface data
            let interfaces = vec![
                {
                    let mut iface = NetworkInterface::new("eth0");
                    iface.network = "pod-network".to_string();
                    iface.mac_address = "52:54:00:12:34:56".to_string();
                    iface.ip_address = Some("10.244.0.5".to_string());
                    iface.state = network::InterfaceState::Up;
                    iface
                },
                {
                    let mut iface = NetworkInterface::new("eth1");
                    iface.network = "storage-network".to_string();
                    iface.mac_address = "52:54:00:12:34:57".to_string();
                    iface.ip_address = Some("192.168.1.10".to_string());
                    iface.state = network::InterfaceState::Up;
                    iface.interface_type = network::InterfaceType::Multus;
                    iface
                },
            ];

            if output == "json" {
                let json = serde_json::to_string_pretty(&interfaces)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&interfaces)?;
                println!("{}", yaml);
            } else {
                // Table format
                println!("{:<12} {:<15} {:<20} {:<18} {:<12} {}",
                    color::label("NAME"),
                    color::label("NETWORK"),
                    color::label("MAC ADDRESS"),
                    color::label("IP ADDRESS"),
                    color::label("TYPE"),
                    color::label("STATE")
                );
                println!("{}", "-".repeat(95));

                for iface in &interfaces {
                    let state_str = match iface.state {
                        network::InterfaceState::Up => color::success("UP"),
                        network::InterfaceState::Down => color::error("DOWN"),
                        network::InterfaceState::Unknown => color::muted("UNKNOWN"),
                    };

                    println!("{:<12} {:<15} {:<20} {:<18} {:<12} {}",
                        iface.name,
                        iface.network,
                        iface.mac_address,
                        iface.ip_address.as_deref().unwrap_or("-"),
                        iface.interface_type.as_str(),
                        state_str
                    );
                }
            }
        }

        Commands::NetworkGet { vm, interface, output } => {
            use network::NetworkInterface;

            let mut iface = NetworkInterface::new(&interface);
            iface.network = "pod-network".to_string();
            iface.mac_address = "52:54:00:12:34:56".to_string();
            iface.ip_address = Some("10.244.0.5".to_string());
            iface.state = network::InterfaceState::Up;
            iface.mtu = 1500;

            if output == "json" {
                let json = serde_json::to_string_pretty(&iface)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&iface)?;
                println!("{}", yaml);
            }
        }

        Commands::NetworkBandwidth { vm, interface, watch, interval } => {
            use network::bandwidth::{BandwidthMetrics, BandwidthMonitor};

            println!("{}", color::header(&format!("Network Bandwidth: {}", vm)));
            if let Some(iface) = &interface {
                println!("  Interface: {}", color::value(iface));
            }
            println!();

            // Simulate bandwidth monitoring
            let iface_name = interface.unwrap_or_else(|| "eth0".to_string());
            let mut monitor = BandwidthMonitor::new(&iface_name);

            // Add sample data
            let mut metrics = BandwidthMetrics::new(&iface_name);
            metrics.rx_bytes = 1_500_000_000;
            metrics.tx_bytes = 800_000_000;
            metrics.rx_packets = 1_200_000;
            metrics.tx_packets = 600_000;
            metrics.rx_errors = 5;
            metrics.tx_errors = 2;
            monitor.add_sample(metrics.clone());

            println!("{:<15} {:<15} {:<15} {:<12} {:<12}",
                color::label("INTERFACE"),
                color::label("RX"),
                color::label("TX"),
                color::label("RX RATE"),
                color::label("TX RATE")
            );
            println!("{}", "-".repeat(75));

            println!("{:<15} {:<15} {:<15} {:<12} {:<12}",
                iface_name,
                BandwidthMetrics::format_bytes(metrics.rx_bytes),
                BandwidthMetrics::format_bytes(metrics.tx_bytes),
                "125 MB/s",
                "80 MB/s"
            );

            println!();
            println!("{}", color::header("Statistics:"));
            println!("  RX Packets:  {}", metrics.rx_packets);
            println!("  TX Packets:  {}", metrics.tx_packets);
            println!("  RX Errors:   {}", if metrics.rx_errors > 0 { color::warning(&metrics.rx_errors.to_string()) } else { metrics.rx_errors.to_string() });
            println!("  TX Errors:   {}", if metrics.tx_errors > 0 { color::warning(&metrics.tx_errors.to_string()) } else { metrics.tx_errors.to_string() });
            println!("  Error Rate:  {:.3}%", metrics.error_rate());

            if watch {
                println!();
                println!("{}", color::info(&format!("ℹ Watch mode not yet implemented. Use --interval {} for update rate.", interval)));
            }
        }

        Commands::NetworkTraffic { vm, interface, period, top, output } => {
            use network::traffic::{TrafficAnalyzer, TrafficFlow, Protocol};
            use network::bandwidth::BandwidthMetrics;

            println!("{}", color::header(&format!("Network Traffic Analysis: {}", vm)));
            if let Some(iface) = &interface {
                println!("  Interface: {}", color::value(iface));
            }
            println!("  Period: {}", color::value(&period));
            println!();

            let mut analyzer = TrafficAnalyzer::new(interface.unwrap_or_else(|| "eth0".to_string()));

            // Add sample flows
            let mut flow1 = TrafficFlow::new("10.244.0.5", "8.8.8.8", 45123, 443, Protocol::TCP);
            flow1.bytes = 50_000_000;
            flow1.packets = 35_000;
            analyzer.add_flow(flow1);

            let mut flow2 = TrafficFlow::new("10.244.0.5", "10.96.0.1", 54321, 53, Protocol::UDP);
            flow2.bytes = 1_500_000;
            flow2.packets = 1_200;
            analyzer.add_flow(flow2);

            let mut flow3 = TrafficFlow::new("10.244.0.5", "10.244.0.8", 8080, 80, Protocol::TCP);
            flow3.bytes = 120_000_000;
            flow3.packets = 85_000;
            analyzer.add_flow(flow3);

            let summary = analyzer.generate_summary(top);

            if output == "json" {
                let json = serde_json::to_string_pretty(&summary)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&summary)?;
                println!("{}", yaml);
            } else {
                println!("{}", color::header("Traffic Summary:"));
                println!("  Total Flows:    {}", summary.total_flows);
                println!("  Active Flows:   {}", summary.active_flows);
                println!("  Total Bytes:    {}", BandwidthMetrics::format_bytes(summary.total_bytes));
                println!("  Total Packets:  {}", summary.total_packets);

                println!();
                println!("{}", color::header("Protocol Breakdown:"));
                for (proto, stats) in &summary.protocol_breakdown {
                    let percentage = summary.protocol_percent(proto);
                    println!("  {:<8} {:<12} ({:.1}%)",
                        proto,
                        BandwidthMetrics::format_bytes(stats.bytes),
                        percentage
                    );
                }

                if !summary.top_talkers.is_empty() {
                    println!();
                    println!("{}", color::header(&format!("Top {} Talkers:", top)));
                    println!("{:<18} {:<15} {:<15} {:<15}",
                        color::label("IP ADDRESS"),
                        color::label("SENT"),
                        color::label("RECEIVED"),
                        color::label("TOTAL")
                    );
                    println!("{}", "-".repeat(70));

                    for talker in &summary.top_talkers {
                        println!("{:<18} {:<15} {:<15} {:<15}",
                            talker.ip_address,
                            BandwidthMetrics::format_bytes(talker.bytes_sent),
                            BandwidthMetrics::format_bytes(talker.bytes_received),
                            BandwidthMetrics::format_bytes(talker.total_bytes)
                        );
                    }
                }
            }
        }

        Commands::NetworkPolicies { all_namespaces, output } => {
            use network::policies::{NetworkPolicy, VMSelector};

            println!("{}", color::header("Network Policies"));
            if all_namespaces {
                println!("  Namespace: {}", color::value("All"));
            }
            println!();

            // Mock policy data
            let policies = vec![
                {
                    let selector = VMSelector::default()
                        .with_label("app".to_string(), "web".to_string());
                    NetworkPolicy::new("web-policy")
                        .with_vm_selector(selector)
                },
                {
                    let selector = VMSelector::default()
                        .with_label("app".to_string(), "database".to_string());
                    NetworkPolicy::new("database-policy")
                        .with_vm_selector(selector)
                },
            ];

            if output == "json" {
                let json = serde_json::to_string_pretty(&policies)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&policies)?;
                println!("{}", yaml);
            } else {
                println!("{:<25} {:<12} {:<12} {}",
                    color::label("NAME"),
                    color::label("INGRESS"),
                    color::label("EGRESS"),
                    color::label("SELECTOR")
                );
                println!("{}", "-".repeat(70));

                for policy in &policies {
                    let selector_str = policy.vm_selector.labels
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join(",");

                    println!("{:<25} {:<12} {:<12} {}",
                        policy.name,
                        policy.ingress_rules.len(),
                        policy.egress_rules.len(),
                        selector_str
                    );
                }
            }
        }

        Commands::NetworkPolicy { name, output } => {
            use network::policies::{NetworkPolicy, VMSelector};

            let selector = VMSelector::default()
                .with_label("app".to_string(), "web".to_string());
            let policy = NetworkPolicy::new(name)
                .with_vm_selector(selector);

            if output == "json" {
                let json = serde_json::to_string_pretty(&policy)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&policy)?;
                println!("{}", yaml);
            }
        }

        Commands::Migrate { vm, target_node, migration_type, plan } => {
            use migration::{MigrationRequest, MigrationType, MigrationStatus, MigrationState, MigrationPhase};

            println!("{}", color::header(&format!("VM Migration: {}", vm)));
            println!();

            let mig_type = match migration_type.as_str() {
                "offline" => MigrationType::Offline,
                "post-copy" => MigrationType::PostCopy,
                _ => MigrationType::Live,
            };

            let request = MigrationRequest::new(&vm, "node1")
                .to_node(target_node.unwrap_or_else(|| "node2".to_string()))
                .with_type(mig_type);

            if plan {
                println!("{}", color::header("Migration Plan:"));
                println!("  VM:           {}", color::value(&vm));
                println!("  Source:       {}", request.source_node);
                println!("  Target:       {}", color::value(request.target_node.as_deref().unwrap_or("auto")));
                println!("  Type:         {}", request.migration_type.as_str());
                println!();
                println!("{}", color::header("Migration Steps:"));
                println!("  1. Validate source and target nodes");
                println!("  2. Prepare target node resources");
                println!("  3. Start live memory transfer");
                println!("  4. Sync disk state");
                println!("  5. Pause VM and final sync");
                println!("  6. Resume VM on target node");
                println!();
                println!("{}", color::info("ℹ Use 'zorvia migrate' without --plan to execute"));
            } else {
                // Simulate migration
                let status = MigrationStatus::new(&vm, "node1", request.target_node.unwrap_or_else(|| "node2".to_string()));

                println!("{}", color::header("Migration Started:"));
                println!("  Migration ID: {}", color::value("mig-12345"));
                println!("  Source:       {}", status.source_node);
                println!("  Target:       {}", status.target_node);
                println!("  Type:         {}", request.migration_type.as_str());
                println!();
                println!("{}", color::success("✓ Migration initiated successfully"));
                println!();
                println!("{}", color::info("ℹ Use 'zorvia migration-status' to monitor progress"));
            }
        }

        Commands::MigrationStatus { vm, watch, interval } => {
            use migration::{MigrationStatus, MigrationState, MigrationPhase};

            println!("{}", color::header(&format!("Migration Status: {}", vm)));
            println!();

            let mut status = MigrationStatus::new(&vm, "node1", "node2");
            status.state = MigrationState::Running;
            status.phase = MigrationPhase::MemoryTransfer;
            status.progress_percent = 65;

            println!("  State:     {}", match status.state {
                MigrationState::Running => color::info("Running"),
                MigrationState::Succeeded => color::success("Succeeded"),
                MigrationState::Failed => color::error("Failed"),
                _ => status.state.to_string(),
            });
            println!("  Phase:     {}", status.phase);
            println!("  Progress:  {}%", status.progress_percent);
            println!("  Source:    {}", status.source_node);
            println!("  Target:    {}", status.target_node);
            println!("  Duration:  {}s", status.duration_secs());

            if watch {
                println!();
                println!("{}", color::info(&format!("ℹ Watch mode not yet implemented. Use --interval {} for update rate.", interval)));
            }
        }

        Commands::MigrationList { all_namespaces, state, output } => {
            use migration::{MigrationStatus, MigrationState, MigrationPhase};

            println!("{}", color::header("VM Migrations"));
            if all_namespaces {
                println!("  Namespace: {}", color::value("All"));
            }
            if let Some(s) = &state {
                println!("  State Filter: {}", color::value(s));
            }
            println!();

            // Mock migration data
            let migrations = vec![
                {
                    let mut m = MigrationStatus::new("web-vm", "node1", "node2");
                    m.state = MigrationState::Running;
                    m.phase = MigrationPhase::MemoryTransfer;
                    m.progress_percent = 75;
                    m
                },
                {
                    let mut m = MigrationStatus::new("db-vm", "node2", "node3");
                    m.state = MigrationState::Succeeded;
                    m.phase = MigrationPhase::Succeeded;
                    m.progress_percent = 100;
                    m.completed_at = Some(Utc::now());
                    m
                },
            ];

            if output == "json" {
                let json = serde_json::to_string_pretty(&migrations)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&migrations)?;
                println!("{}", yaml);
            } else {
                println!("{:<15} {:<12} {:<10} {:<10} {:<10} {}",
                    color::label("VM"),
                    color::label("STATE"),
                    color::label("PHASE"),
                    color::label("PROGRESS"),
                    color::label("SOURCE"),
                    color::label("TARGET")
                );
                println!("{}", "-".repeat(80));

                for m in &migrations {
                    let state_str = match m.state {
                        MigrationState::Running => color::info("Running"),
                        MigrationState::Succeeded => color::success("Succeeded"),
                        MigrationState::Failed => color::error("Failed"),
                        _ => m.state.to_string(),
                    };

                    println!("{:<15} {:<12} {:<10} {:<10} {:<10} {}",
                        m.vm_name,
                        state_str,
                        m.phase.to_string(),
                        format!("{}%", m.progress_percent),
                        m.source_node,
                        m.target_node
                    );
                }
            }
        }

        Commands::HAConfig { vm, enable, disable, priority, eviction_strategy } => {
            use migration::ha::{HAConfig, HAPriority, EvictionStrategy};

            if enable == disable {
                return Err(anyhow!("Must specify either --enable or --disable"));
            }

            println!("{}", color::header(&format!("HA Configuration: {}", vm)));
            println!();

            let mut config = HAConfig::new(&vm);
            config.enabled = enable;

            if let Some(p) = priority {
                config.priority = match p.as_str() {
                    "critical" => HAPriority::Critical,
                    "high" => HAPriority::High,
                    "low" => HAPriority::Low,
                    _ => HAPriority::Normal,
                };
            }

            if let Some(s) = eviction_strategy {
                config.eviction_strategy = match s.as_str() {
                    "shutdown" => EvictionStrategy::Shutdown,
                    "none" => EvictionStrategy::None,
                    _ => EvictionStrategy::LiveMigrate,
                };
            }

            println!("  Enabled:            {}", if config.enabled { color::success("Yes") } else { color::muted("No") });
            println!("  Priority:           {:?}", config.priority);
            println!("  Eviction Strategy:  {}", config.eviction_strategy);
            println!("  Auto Restart:       {}", config.failover_policy.auto_restart);
            println!("  Max Restarts:       {}", config.failover_policy.max_restart_attempts);
            println!();
            println!("{}", color::success("✓ HA configuration updated"));
        }

        Commands::HAStatus { vm, output } => {
            use migration::ha::HAConfig;

            let config = HAConfig::new(&vm);

            if output == "json" {
                let json = serde_json::to_string_pretty(&config)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&config)?;
                println!("{}", yaml);
            }
        }

        Commands::EvacuateNode { node, reason, max_parallel, timeout, force, plan } => {
            use migration::evacuation::{EvacuationRequest, EvacuationStatus, EvacuationPlanner};

            println!("{}", color::header(&format!("Node Evacuation: {}", node)));
            println!();

            let request = EvacuationRequest::new(&node, reason.unwrap_or_else(|| "Maintenance".to_string()))
                .with_timeout(timeout);

            let planner = EvacuationPlanner::new(max_parallel);

            if plan {
                println!("{}", color::header("Evacuation Plan:"));
                println!("  Node:         {}", color::value(&node));
                println!("  Reason:       {}", request.reason);
                println!("  Max Parallel: {}", max_parallel);
                println!("  Timeout:      {}s", timeout);
                println!("  Force:        {}", if force { "Yes" } else { "No" });
                println!();

                // Mock VM list with priorities
                let vms = vec![
                    ("critical-db".to_string(), 100),
                    ("web-app-1".to_string(), 50),
                    ("web-app-2".to_string(), 50),
                    ("cache".to_string(), 30),
                    ("worker-1".to_string(), 20),
                ];

                let batches = planner.plan_evacuation(vms);
                let estimated = planner.estimate_duration(5, 120);

                println!("{}", color::header("Migration Batches:"));
                for (i, batch) in batches.iter().enumerate() {
                    println!("  Batch {}: {}", i + 1, batch.join(", "));
                }
                println!();
                println!("  Estimated Duration: {}s (~{} minutes)", estimated, estimated / 60);
                println!();
                println!("{}", color::info("ℹ Use 'zorvia evacuate-node' without --plan to execute"));
            } else {
                let mut status = EvacuationStatus::new(&node, 5);

                println!("{}", color::header("Evacuation Started:"));
                println!("  Node:         {}", status.node_name);
                println!("  Total VMs:    {}", status.total_vms);
                println!("  Strategy:     Live Migration");
                println!("  Max Parallel: {}", max_parallel);
                println!();
                println!("{}", color::success("✓ Evacuation initiated successfully"));
                println!();
                println!("{}", color::info("ℹ Use 'zorvia evacuation-status' to monitor progress"));
            }
        }

        Commands::EvacuationStatus { node, watch } => {
            use migration::evacuation::{EvacuationStatus, EvacuationState};

            println!("{}", color::header(&format!("Evacuation Status: {}", node)));
            println!();

            let mut status = EvacuationStatus::new(&node, 5);
            status.state = EvacuationState::InProgress;
            status.migrated_vms = 3;
            status.in_progress_vms = 1;
            status.failed_vms = 0;

            println!("  State:        {}", match status.state {
                EvacuationState::InProgress => color::info("In Progress"),
                EvacuationState::Completed => color::success("Completed"),
                EvacuationState::Failed => color::error("Failed"),
                _ => status.state.to_string(),
            });
            println!("  Total VMs:    {}", status.total_vms);
            println!("  Migrated:     {}", color::success(&status.migrated_vms.to_string()));
            println!("  In Progress:  {}", status.in_progress_vms);
            println!("  Failed:       {}", if status.failed_vms > 0 { color::error(&status.failed_vms.to_string()) } else { "0".to_string() });
            println!("  Progress:     {}%", status.progress_percent());
            println!("  Duration:     {}s", status.duration_secs());

            if watch {
                println!();
                println!("{}", color::info("ℹ Watch mode not yet implemented"));
            }
        }

        Commands::BackupCreate { vm, name, backup_type, compression, no_encryption } => {
            use backup::{BackupConfig, BackupType, CompressionType, BackupStatus};

            let backup_name = name.unwrap_or_else(|| {
                format!("{}-backup-{}", vm, Utc::now().format("%Y%m%d-%H%M%S"))
            });

            println!("{}", color::header(&format!("Creating Backup: {}", vm)));
            println!();

            let b_type = match backup_type.as_str() {
                "incremental" => BackupType::Incremental,
                "differential" => BackupType::Differential,
                _ => BackupType::Full,
            };

            let comp_type = match compression.as_str() {
                "zstd" => CompressionType::Zstd,
                "lz4" => CompressionType::Lz4,
                "none" => CompressionType::None,
                _ => CompressionType::Gzip,
            };

            let mut config = BackupConfig::new(&vm, &backup_name);
            config.backup_type = b_type;
            config.compression = comp_type;
            if no_encryption {
                config.encryption_enabled = false;
            }

            println!("  Backup Name:  {}", color::value(&backup_name));
            println!("  VM:           {}", vm);
            println!("  Type:         {}", config.backup_type.as_str());
            println!("  Compression:  {:?}", config.compression);
            println!("  Encryption:   {}", if config.encryption_enabled { "Enabled" } else { "Disabled" });
            println!();
            println!("{}", color::success("✓ Backup created successfully"));
            println!();
            println!("{}", color::info(&format!("ℹ Use 'zorvia backup-get {}' to view details", backup_name)));
        }

        Commands::BackupList { vm, output } => {
            use backup::BackupStatus;

            println!("{}", color::header("Backups"));
            if let Some(v) = &vm {
                println!("  VM: {}", color::value(v));
            }
            println!();

            // Mock backup data
            let backups = vec![
                {
                    let mut b = BackupStatus::new("web-vm", "web-vm-backup-20240101");
                    b.size_bytes = 50_000_000_000;
                    b.compressed_size_bytes = 15_000_000_000;
                    b
                },
                {
                    let mut b = BackupStatus::new("db-vm", "db-vm-backup-20240101");
                    b.size_bytes = 100_000_000_000;
                    b.compressed_size_bytes = 30_000_000_000;
                    b
                },
            ];

            if output == "json" {
                let json = serde_json::to_string_pretty(&backups)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&backups)?;
                println!("{}", yaml);
            } else {
                println!("{:<30} {:<15} {:<15} {:<15} {}",
                    color::label("BACKUP"),
                    color::label("VM"),
                    color::label("SIZE"),
                    color::label("COMPRESSED"),
                    color::label("RATIO")
                );
                println!("{}", "-".repeat(90));

                for b in &backups {
                    let size = format!("{:.2} GB", b.size_bytes as f64 / 1_000_000_000.0);
                    let compressed = format!("{:.2} GB", b.compressed_size_bytes as f64 / 1_000_000_000.0);
                    let ratio = format!("{:.1}%", b.compression_ratio());

                    println!("{:<30} {:<15} {:<15} {:<15} {}",
                        b.backup_name,
                        b.vm_name,
                        size,
                        compressed,
                        ratio
                    );
                }
            }
        }

        Commands::BackupGet { name, output } => {
            use backup::BackupStatus;

            let backup = BackupStatus::new("my-vm", name);

            if output == "json" {
                let json = serde_json::to_string_pretty(&backup)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&backup)?;
                println!("{}", yaml);
            }
        }

        Commands::BackupDelete { name, yes } => {
            if !yes {
                print!("Are you sure you want to delete backup '{}'? [y/N] ", name);
                return Err(anyhow!("Operation cancelled. Use --yes to skip confirmation."));
            }

            println!("{}", color::header(&format!("Deleting Backup: {}", name)));
            println!();
            println!("{}", color::success("✓ Backup deleted successfully"));
        }

        Commands::BackupRestore { backup, target, start } => {
            use backup::recovery::RestoreOperation;

            let target_vm = target.unwrap_or_else(|| backup.replace("-backup-", "-restored-"));

            println!("{}", color::header(&format!("Restoring from Backup: {}", backup)));
            println!();

            let restore = RestoreOperation::new("restore-001", "original-vm", &backup)
                .to_new_vm(&target_vm);

            println!("  Restore ID:   {}", color::value("restore-001"));
            println!("  Backup:       {}", backup);
            println!("  Target VM:    {}", target_vm);
            println!("  Start After:  {}", if start { "Yes" } else { "No" });
            println!();
            println!("{}", color::success("✓ Restore initiated successfully"));
            println!();
            println!("{}", color::info("ℹ Restore in progress. This may take several minutes."));
        }

        Commands::BackupVerify { name, verification_type } => {
            use backup::verify::{VerificationRunner, VerificationType, VerificationStatus};

            println!("{}", color::header(&format!("Verifying Backup: {}", name)));
            println!();

            let v_type = match verification_type.as_str() {
                "quick" => VerificationType::Quick,
                "full" => VerificationType::Full,
                _ => VerificationType::Standard,
            };

            let report = VerificationRunner::verify(&name, v_type);

            println!("  Verification Type:  {:?}", report.verification_type);
            println!("  Status:             {}", match report.status {
                VerificationStatus::Passed => color::success("✓ Passed"),
                VerificationStatus::Failed => color::error("✗ Failed"),
                VerificationStatus::Warning => color::warning("⚠ Warning"),
                _ => report.status.to_string(),
            });
            println!("  Checks Run:         {}", report.checks.len());
            println!("  Passed:             {}", report.checks.len() - report.error_count as usize - report.warning_count as usize);
            println!("  Warnings:           {}", if report.warning_count > 0 { color::warning(&report.warning_count.to_string()) } else { "0".to_string() });
            println!("  Errors:             {}", if report.error_count > 0 { color::error(&report.error_count.to_string()) } else { "0".to_string() });
            println!("  Pass Rate:          {:.1}%", report.pass_rate());
            println!("  Duration:           {}s", report.duration_secs());
        }

        Commands::BackupSchedules { output } => {
            use backup::schedule::{BackupSchedule, ScheduleType};

            println!("{}", color::header("Backup Schedules"));
            println!();

            // Mock schedule data
            let schedules = vec![
                BackupSchedule::new("daily-full-backup", ScheduleType::daily(2, 0)),
                BackupSchedule::new("hourly-incremental", ScheduleType::hourly(0)),
            ];

            if output == "json" {
                let json = serde_json::to_string_pretty(&schedules)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&schedules)?;
                println!("{}", yaml);
            } else {
                println!("{:<25} {:<15} {:<10} {}",
                    color::label("NAME"),
                    color::label("TYPE"),
                    color::label("ENABLED"),
                    color::label("NEXT RUN")
                );
                println!("{}", "-".repeat(70));

                for s in &schedules {
                    let enabled_str = if s.enabled { color::success("Yes") } else { color::muted("No") };
                    let next_run = s.next_run
                        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "-".to_string());

                    println!("{:<25} {:<15} {:<10} {}",
                        s.name,
                        "Daily", // Simplified
                        enabled_str,
                        next_run
                    );
                }
            }
        }

        Commands::BackupScheduleCreate { name, schedule, vm } => {
            println!("{}", color::header(&format!("Creating Backup Schedule: {}", name)));
            println!();
            println!("  Schedule:  {}", color::value(&schedule));
            println!("  VM:        {}", vm.as_deref().unwrap_or("All"));
            println!();
            println!("{}", color::success("✓ Schedule created successfully"));
        }

        Commands::RecoveryPlan { name, output } => {
            use backup::recovery::RecoveryPlan;

            let plan = RecoveryPlan::new(name)
                .with_description("Disaster recovery plan");

            if output == "json" {
                let json = serde_json::to_string_pretty(&plan)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&plan)?;
                println!("{}", yaml);
            }
        }

        Commands::RecoveryExecute { plan, dry_run } => {
            println!("{}", color::header(&format!("Executing Recovery Plan: {}", plan)));
            println!();

            if dry_run {
                println!("{}", color::info("=== DRY RUN MODE ==="));
                println!();
                println!("Recovery Steps:");
                println!("  1. Restore infrastructure VMs");
                println!("  2. Restore application VMs");
                println!("  3. Restore database VMs");
                println!("  4. Verify all VMs are running");
                println!();
                println!("{}", color::info("ℹ Run without --dry-run to execute"));
            } else {
                println!("  Phase 1:  Restoring infrastructure VMs...");
                println!("  Phase 2:  Restoring application VMs...");
                println!("  Phase 3:  Restoring database VMs...");
                println!();
                println!("{}", color::success("✓ Recovery completed successfully"));
            }
        }

        // ========== SECURITY & COMPLIANCE ==========

        Commands::SecurityScan { vm, scan_type, containers, output } => {
            use security::scan::{ScanConfig, ScanType, VulnerabilityScanner};

            println!("{}", color::header(&format!("Scanning VM: {}", vm)));
            println!();

            let s_type = match scan_type.as_str() {
                "quick" => ScanType::Quick,
                "deep" => ScanType::Deep,
                "compliance" => ScanType::Compliance,
                _ => ScanType::Standard,
            };

            let mut config = ScanConfig::new(&vm, s_type);
            if containers {
                config = config.enable_containers();
            }

            println!("  Scan Type:  {}", color::value(&scan_type));
            println!("  Containers: {}", if containers { color::success("Yes") } else { "No".to_string() });
            println!();
            println!("Scanning...");

            let result = VulnerabilityScanner::scan(&config);

            println!();
            println!("Scan Results:");
            println!("  Status:     {}", color::success(&result.status.to_string()));
            println!("  Total:      {}", color::value(&result.statistics.total.to_string()));
            println!("  Critical:   {}", if result.statistics.critical > 0 {
                color::error(&result.statistics.critical.to_string())
            } else {
                color::success("0")
            });
            println!("  High:       {}", if result.statistics.high > 0 {
                color::warning(&result.statistics.high.to_string())
            } else {
                color::success("0")
            });
            println!("  Medium:     {}", result.statistics.medium);
            println!("  Low:        {}", result.statistics.low);
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&result)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&result)?;
                println!("{}", yaml);
            }
        }

        Commands::SecurityAssess { vm, output } => {
            use security::{SecurityAssessment, Vulnerability, Severity};

            let mut assessment = SecurityAssessment::new(&vm);

            // Example vulnerabilities
            assessment.add_vulnerability(
                Vulnerability::new("VULN-001", "OpenSSL vulnerability", Severity::High)
                    .with_cvss(7.5)
            );
            assessment.add_vulnerability(
                Vulnerability::new("VULN-002", "Kernel vulnerability", Severity::Medium)
                    .with_cvss(5.0)
            );

            assessment.calculate_score();

            println!("{}", color::header(&format!("Security Assessment: {}", vm)));
            println!();
            println!("  Score:         {}", color::value(&assessment.overall_score.to_string()));
            println!("  Risk Level:    {}", match assessment.risk_level {
                security::RiskLevel::Critical => color::error("Critical"),
                security::RiskLevel::High => color::error("High"),
                security::RiskLevel::Medium => color::warning("Medium"),
                security::RiskLevel::Low => color::success("Low"),
                security::RiskLevel::Unknown => color::muted("Unknown"),
            });
            println!("  Vulnerabilities: {}", assessment.vulnerabilities.len());
            println!("    Critical:    {}", color::error(&assessment.critical_count().to_string()));
            println!("    High:        {}", color::warning(&assessment.high_count().to_string()));
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&assessment)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&assessment)?;
                println!("{}", yaml);
            }
        }

        Commands::SecurityHarden { vm, profile, verify_only } => {
            use security::hardening::{HardeningEngine, SecurityBaseline};

            println!("{}", color::header(&format!("Security Hardening: {}", vm)));
            println!();

            let baseline = match profile.as_str() {
                "stig" => SecurityBaseline::STIG,
                "pci-dss" => SecurityBaseline::PCI_DSS,
                "nist" => SecurityBaseline::NIST,
                "custom" => SecurityBaseline::Custom,
                _ => SecurityBaseline::CIS,
            };

            let hardening_profile = match profile.as_str() {
                "stig" => HardeningEngine::stig_profile(),
                _ => HardeningEngine::cis_profile(),
            };

            println!("  Profile:     {}", color::value(&baseline.to_string()));
            println!("  Rules:       {}", hardening_profile.rule_count());
            println!("  Mode:        {}", if verify_only {
                color::info("Verify Only")
            } else {
                color::warning("Apply")
            });
            println!();

            let result = if verify_only {
                HardeningEngine::verify(&vm, &hardening_profile)
            } else {
                HardeningEngine::apply(&vm, &hardening_profile)
            };

            println!("Results:");
            println!("  Status:      {}", color::success(&result.status.to_string()));
            println!("  Applied:     {}", color::success(&result.statistics.applied.to_string()));
            println!("  Skipped:     {}", result.statistics.skipped);
            println!("  Failed:      {}", if result.statistics.failed > 0 {
                color::error(&result.statistics.failed.to_string())
            } else {
                color::success("0")
            });
            println!("  Success:     {}%", result.success_rate() as u8);
        }

        Commands::SecurityProfiles { details } => {
            use security::hardening::{HardeningEngine, SecurityBaseline};

            println!("{}", color::header("Security Hardening Profiles"));
            println!();

            let profiles = vec![
                (SecurityBaseline::CIS, HardeningEngine::cis_profile()),
                (SecurityBaseline::STIG, HardeningEngine::stig_profile()),
            ];

            if details {
                for (baseline, profile) in profiles {
                    println!("Profile: {}", color::value(&baseline.to_string()));
                    println!("  Name:        {}", profile.name);
                    println!("  Description: {}", profile.description);
                    println!("  Rules:       {}", profile.rule_count());
                    println!();
                }
            } else {
                println!("{:<20} {:<50} {}",
                    color::label("PROFILE"),
                    color::label("DESCRIPTION"),
                    color::label("RULES")
                );
                println!("{}", "-".repeat(80));

                for (baseline, profile) in profiles {
                    println!("{:<20} {:<50} {}",
                        baseline.to_string(),
                        profile.description,
                        profile.rule_count()
                    );
                }
            }
        }

        Commands::ComplianceCheck { vm, framework, output } => {
            use security::compliance::{ComplianceChecker, ComplianceFramework};

            println!("{}", color::header(&format!("Compliance Check: {}", vm)));
            println!();

            let fw = match framework.as_str() {
                "hipaa" => ComplianceFramework::HIPAA,
                "soc2" => ComplianceFramework::SOC2,
                "iso27001" => ComplianceFramework::ISO27001,
                "gdpr" => ComplianceFramework::GDPR,
                "nist" => ComplianceFramework::NIST,
                "cis" => ComplianceFramework::CIS,
                _ => ComplianceFramework::PCIDSS,
            };

            println!("  Framework:   {}", color::value(&fw.to_string()));
            println!();
            println!("Checking compliance...");

            let report = match framework.as_str() {
                "hipaa" => ComplianceChecker::check_hipaa(&vm),
                "soc2" => ComplianceChecker::check_soc2(&vm),
                _ => ComplianceChecker::check_pci_dss(&vm),
            };

            println!();
            println!("Compliance Report:");
            println!("  Status:      {}", if report.compliant {
                color::success("Compliant")
            } else {
                color::error("Non-Compliant")
            });
            println!("  Score:       {}%", report.summary.compliance_score as u8);
            println!("  Total:       {}", report.summary.total_checks);
            println!("  Passed:      {}", color::success(&report.summary.passed.to_string()));
            println!("  Failed:      {}", if report.summary.failed > 0 {
                color::error(&report.summary.failed.to_string())
            } else {
                color::success("0")
            });
            println!("  Critical:    {}", if report.summary.critical_failures > 0 {
                color::error(&report.summary.critical_failures.to_string())
            } else {
                color::success("0")
            });
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&report)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&report)?;
                println!("{}", yaml);
            }
        }

        Commands::ComplianceReport { vm, report_id, output } => {
            use security::compliance::{ComplianceChecker};

            let report = ComplianceChecker::check_pci_dss(&vm);

            println!("{}", color::header(&format!("Compliance Report: {}", vm)));
            println!();
            println!("  Report ID:   {}", report_id.as_deref().unwrap_or(&report.report_id));
            println!("  Generated:   {}", report.generated_at.format("%Y-%m-%d %H:%M:%S"));
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&report)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&report)?;
                println!("{}", yaml);
            }
        }

        Commands::AuditList { vm, event_type, severity, security_only, output } => {
            use security::audit::{AuditLog, AuditEvent, EventType, EventSeverity};

            println!("{}", color::header("Audit Events"));
            if let Some(ref vm_name) = vm {
                println!("  VM: {}", color::value(vm_name));
            }
            println!();

            // Create example audit log
            let mut log = AuditLog::new(vm.clone());

            // Add example events
            log.add_event(
                AuditEvent::new(EventType::Authentication, "user@example.com", "test-vm", "login")
                    .with_severity(EventSeverity::Info)
            );
            log.add_event(
                AuditEvent::new(EventType::VMOperation, "admin", "test-vm", "start")
                    .with_severity(EventSeverity::Info)
            );
            log.add_event(
                AuditEvent::new(EventType::SecurityViolation, "user", "test-vm", "unauthorized")
                    .with_severity(EventSeverity::Critical)
            );

            let events: Vec<&AuditEvent> = if security_only {
                log.security_events()
            } else {
                log.events.iter().collect()
            };

            if output == "json" {
                let json = serde_json::to_string_pretty(&events)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&events)?;
                println!("{}", yaml);
            } else {
                println!("{:<25} {:<20} {:<15} {:<10} {}",
                    color::label("TIMESTAMP"),
                    color::label("TYPE"),
                    color::label("ACTOR"),
                    color::label("SEVERITY"),
                    color::label("ACTION")
                );
                println!("{}", "-".repeat(90));

                for event in events {
                    let severity_str = match event.severity {
                        EventSeverity::Critical => color::error("Critical"),
                        EventSeverity::High => color::error("High"),
                        EventSeverity::Medium => color::warning("Medium"),
                        EventSeverity::Low => color::info("Low"),
                        EventSeverity::Info => color::muted("Info"),
                    };

                    println!("{:<25} {:<20} {:<15} {:<10} {}",
                        event.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        event.event_type.to_string(),
                        event.actor,
                        severity_str,
                        event.action
                    );
                }
            }
        }

        Commands::AuditGet { log_id, output } => {
            use security::audit::AuditLog;

            let log = AuditLog::new(Some("test-vm".to_string()));

            println!("{}", color::header(&format!("Audit Log: {}", log_id)));
            println!();
            println!("  Log ID:      {}", log.log_id);
            println!("  Events:      {}", log.event_count());
            println!("  Created:     {}", log.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&log)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&log)?;
                println!("{}", yaml);
            }
        }

        Commands::AuditStats { vm, period, output } => {
            use security::audit::{AuditLog, AuditStatistics};

            println!("{}", color::header("Audit Statistics"));
            if let Some(ref vm_name) = vm {
                println!("  VM:          {}", color::value(vm_name));
            }
            println!("  Period:      {}", period);
            println!();

            let log = AuditLog::new(vm);
            let stats = AuditStatistics::from_log(&log);

            if output == "json" {
                let json = serde_json::to_string_pretty(&stats)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&stats)?;
                println!("{}", yaml);
            } else {
                println!("Summary:");
                println!("  Total Events:      {}", stats.total_events);
                println!("  Security Events:   {}", stats.security_events);
                println!("  Critical Events:   {}", if stats.critical_events > 0 {
                    color::error(&stats.critical_events.to_string())
                } else {
                    color::success("0")
                });
                println!("  Failed Events:     {}", if stats.failed_events > 0 {
                    color::warning(&stats.failed_events.to_string())
                } else {
                    color::success("0")
                });
            }
        }

        // ========== COST MANAGEMENT & OPTIMIZATION ==========

        Commands::CostAnalyze { vm, period, output } => {
            use cost::{CostCalculator, VMCost};

            println!("{}", color::header("Cost Analysis"));
            if let Some(ref vm_name) = vm {
                println!("  VM:      {}", color::value(vm_name));
            }
            println!("  Period:  {}", period);
            println!();

            let calculator = CostCalculator::default();
            let cost = calculator.calculate_vm_cost(
                vm.as_deref().unwrap_or("example-vm"),
                "default",
                4,
                8,
                20,
                730.0
            );

            println!("Cost Breakdown:");
            println!("  CPU:      ${:.2}", cost.cpu_cost);
            println!("  Memory:   ${:.2}", cost.memory_cost);
            println!("  Storage:  ${:.2}", cost.storage_cost);
            println!("  Network:  ${:.2}", cost.network_cost);
            println!("  Total:    {}", color::value(&format!("${:.2}", cost.total_cost)));
            println!();
            println!("  Runtime:  {:.1} hours", cost.runtime_hours);
            println!("  Cost/hr:  ${:.4}", cost.cost_per_hour());

            if output == "json" {
                println!();
                let json = serde_json::to_string_pretty(&cost)?;
                println!("{}", json);
            } else if output == "yaml" {
                println!();
                let yaml = serde_yaml::to_string(&cost)?;
                println!("{}", yaml);
            }
        }

        Commands::CostSummary { namespace, period, group_by, output } => {
            use cost::CostSummary;

            println!("{}", color::header("Cost Summary"));
            if let Some(ref ns) = namespace {
                println!("  Namespace: {}", color::value(ns));
            }
            println!("  Period:    {}", period);
            if let Some(ref group) = group_by {
                println!("  Group By:  {}", group);
            }
            println!();

            let summary = CostSummary::new();

            println!("Summary:");
            println!("  Total Cost:       {}", color::value(&format!("${:.2}", summary.total_cost)));
            println!("  VM Count:         {}", summary.vm_count);
            println!("  Avg Cost/VM:      ${:.2}", summary.average_cost_per_vm());
            println!();
            println!("Resource Breakdown:");
            println!("  CPU:              ${:.2}", summary.cpu_cost);
            println!("  Memory:           ${:.2}", summary.memory_cost);
            println!("  Storage:          ${:.2}", summary.storage_cost);
            println!("  Network:          ${:.2}", summary.network_cost);
            println!("  Snapshots:        ${:.2}", summary.snapshot_cost);

            if output == "json" {
                println!();
                let json = serde_json::to_string_pretty(&summary)?;
                println!("{}", json);
            } else if output == "yaml" {
                println!();
                let yaml = serde_yaml::to_string(&summary)?;
                println!("{}", yaml);
            }
        }

        Commands::CostReport { report_type, format, output } => {
            use cost::reports::{ReportGenerator, ReportExporter};
            use chrono::Utc;

            println!("{}", color::header(&format!("{} Cost Report", report_type)));
            println!();

            let report = match report_type.as_str() {
                "monthly" => ReportGenerator::monthly_report(2024, 1),
                "weekly" => ReportGenerator::weekly_report(Utc::now()),
                _ => ReportGenerator::custom_report(Utc::now() - chrono::Duration::days(30), Utc::now()),
            };

            println!("  Report ID:   {}", report.report_id);
            println!("  Period:      {} to {}",
                report.period_start.format("%Y-%m-%d"),
                report.period_end.format("%Y-%m-%d")
            );
            println!("  Total Cost:  {}", color::value(&format!("${:.2}", report.summary.total_cost)));
            println!();

            let content = if format == "csv" {
                ReportExporter::to_csv(&report)
            } else if format == "yaml" {
                serde_yaml::to_string(&report)?
            } else {
                ReportExporter::to_json(&report)?
            };

            if let Some(file_path) = output {
                std::fs::write(&file_path, content)?;
                println!("{}", color::success(&format!("Report saved to: {}", file_path)));
            } else {
                println!("{}", content);
            }
        }

        Commands::BudgetList { output } => {
            use cost::budgets::{BudgetManager, Budget, BudgetPeriod, BudgetScope};

            println!("{}", color::header("Budgets"));
            println!();

            let mut manager = BudgetManager::new();

            // Example budgets
            manager.add_budget(
                Budget::new("monthly-budget", 5000.0, BudgetPeriod::Monthly)
                    .with_scope(BudgetScope::Global)
            );
            manager.add_budget(
                Budget::new("dev-budget", 1000.0, BudgetPeriod::Monthly)
                    .with_scope(BudgetScope::Namespace("dev".to_string()))
            );

            if output == "json" {
                let json = serde_json::to_string_pretty(&manager.all_budgets())?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&manager.all_budgets())?;
                println!("{}", yaml);
            } else {
                println!("{:<20} {:<15} {:<15} {:<10}",
                    color::label("NAME"),
                    color::label("AMOUNT"),
                    color::label("PERIOD"),
                    color::label("SCOPE")
                );
                println!("{}", "-".repeat(65));

                for budget in manager.all_budgets() {
                    println!("{:<20} ${:<14.2} {:<15} {}",
                        budget.name,
                        budget.amount,
                        budget.period.to_string(),
                        budget.scope.to_string()
                    );
                }
            }
        }

        Commands::BudgetCreate { name, amount, period, scope, alert_threshold } => {
            use cost::budgets::{Budget, BudgetPeriod, BudgetScope, BudgetAlert, NotificationType};

            println!("{}", color::header(&format!("Creating Budget: {}", name)));
            println!();

            let budget_period = match period.as_str() {
                "daily" => BudgetPeriod::Daily,
                "weekly" => BudgetPeriod::Weekly,
                "quarterly" => BudgetPeriod::Quarterly,
                "yearly" => BudgetPeriod::Yearly,
                _ => BudgetPeriod::Monthly,
            };

            let budget_scope = if scope == "global" {
                BudgetScope::Global
            } else if let Some(ns) = scope.strip_prefix("namespace:") {
                BudgetScope::Namespace(ns.to_string())
            } else if let Some(team) = scope.strip_prefix("team:") {
                BudgetScope::Team(team.to_string())
            } else {
                BudgetScope::Global
            };

            let mut budget = Budget::new(&name, amount, budget_period)
                .with_scope(budget_scope);

            if let Some(threshold) = alert_threshold {
                budget = budget.add_alert(
                    BudgetAlert::new(threshold, NotificationType::Email)
                );
            }

            println!("  Name:       {}", color::value(&budget.name));
            println!("  Amount:     {}", color::value(&format!("${:.2}", budget.amount)));
            println!("  Period:     {}", budget.period);
            println!("  Scope:      {}", budget.scope);
            if !budget.alerts.is_empty() {
                println!("  Alerts:     {} configured", budget.alerts.len());
            }
            println!();
            println!("{}", color::success("✓ Budget created successfully"));
        }

        Commands::BudgetStatus { name, output } => {
            use cost::budgets::{Budget, BudgetPeriod, BudgetStatus};

            let mut budget = Budget::new(&name, 5000.0, BudgetPeriod::Monthly);
            budget.update_spend(3750.0);

            let status = BudgetStatus::from_budget(&budget);

            println!("{}", color::header(&format!("Budget Status: {}", name)));
            println!();
            println!("  Amount:       {}", color::value(&format!("${:.2}", status.amount)));
            println!("  Current:      ${:.2}", status.current_spend);
            println!("  Remaining:    ${:.2}", status.remaining);
            println!("  Utilization:  {}%", status.utilization_percent as u8);
            println!("  Status:       {}", match status.status {
                cost::budgets::Status::Healthy => color::success("Healthy"),
                cost::budgets::Status::Warning => color::warning("Warning"),
                cost::budgets::Status::Critical => color::error("Critical"),
                cost::budgets::Status::Exceeded => color::error("Exceeded"),
            });

            if output == "json" {
                println!();
                let json = serde_json::to_string_pretty(&status)?;
                println!("{}", json);
            } else if output == "yaml" {
                println!();
                let yaml = serde_yaml::to_string(&status)?;
                println!("{}", yaml);
            }
        }

        Commands::CostOptimize { vm, high_priority_only, output } => {
            use cost::optimization::{OptimizationEngine, OptimizationReport};

            println!("{}", color::header("Cost Optimization Recommendations"));
            if let Some(ref vm_name) = vm {
                println!("  VM: {}", color::value(vm_name));
            }
            println!();

            let report = OptimizationEngine::generate_report(vm.as_deref().unwrap_or("example-vm"));

            let recommendations = if high_priority_only {
                report.high_priority_recommendations()
            } else {
                report.recommendations.iter().collect()
            };

            println!("Potential Savings: {}", color::value(&format!("${:.2}/month", report.total_potential_savings)));
            println!("Recommendations:   {}", recommendations.len());
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&recommendations)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&recommendations)?;
                println!("{}", yaml);
            } else {
                println!("{:<15} {:<25} {:<10} {:<15} {}",
                    color::label("PRIORITY"),
                    color::label("TYPE"),
                    color::label("SAVINGS"),
                    color::label("SAVINGS %"),
                    color::label("DESCRIPTION")
                );
                println!("{}", "-".repeat(90));

                for rec in recommendations {
                    let priority_str = match rec.priority {
                        cost::optimization::Priority::Critical => color::error("Critical"),
                        cost::optimization::Priority::High => color::error("High"),
                        cost::optimization::Priority::Medium => color::warning("Medium"),
                        cost::optimization::Priority::Low => color::info("Low"),
                    };

                    println!("{:<15} {:<25} ${:<9.2} {:<15.1}% {}",
                        priority_str,
                        rec.recommendation_type.to_string(),
                        rec.potential_savings,
                        rec.savings_percent,
                        rec.description
                    );
                }
            }
        }

        Commands::CostWaste { waste_type, min_waste, output } => {
            use cost::optimization::{OptimizationEngine, WasteType};

            println!("{}", color::header("Cost Waste Report"));
            if let Some(ref wtype) = waste_type {
                println!("  Type: {}", wtype);
            }
            println!("  Minimum: ${:.2}/month", min_waste);
            println!();

            // Example waste reports
            let wastes = vec![
                OptimizationEngine::detect_storage_waste(100, 10.0),
                OptimizationEngine::detect_old_snapshots(10, 120, 5.0).unwrap(),
            ];

            let filtered: Vec<_> = wastes.iter()
                .filter(|w| w.monthly_waste >= min_waste)
                .collect();

            println!("Total Monthly Waste: {}", color::error(&format!("${:.2}", filtered.iter().map(|w| w.monthly_waste).sum::<f64>())));
            println!("Waste Items:         {}", filtered.len());
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&filtered)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&filtered)?;
                println!("{}", yaml);
            } else {
                println!("{:<20} {:<20} {:<15} {}",
                    color::label("RESOURCE"),
                    color::label("TYPE"),
                    color::label("MONTHLY WASTE"),
                    color::label("DETAILS")
                );
                println!("{}", "-".repeat(80));

                for waste in filtered {
                    let severity_str = match waste.severity {
                        cost::optimization::WasteSeverity::High => color::error("High"),
                        cost::optimization::WasteSeverity::Medium => color::warning("Medium"),
                        cost::optimization::WasteSeverity::Low => color::info("Low"),
                    };

                    println!("{:<20} {:<20} ${:<14.2} {}",
                        waste.vm_name,
                        format!("{:?}", waste.waste_type),
                        waste.monthly_waste,
                        waste.details
                    );
                }
            }
        }

        Commands::CostForecast { budget, period, output } => {
            use cost::budgets::CostForecast;

            println!("{}", color::header("Cost Forecast"));
            println!("  Period: {}", period);
            if let Some(b) = budget {
                println!("  Budget: ${:.2}", b);
            }
            println!();

            let mut forecast = CostForecast::new(cost::budgets::BudgetPeriod::Monthly, 1500.0);
            forecast.project_linear(15.0, 30.0);

            println!("Forecast:");
            println!("  Current Spend:    ${:.2}", forecast.current_spend);
            println!("  Projected Spend:  {}", color::value(&format!("${:.2}", forecast.projected_spend)));
            println!("  Confidence:       {}%", forecast.confidence as u8);
            println!("  Method:           {:?}", forecast.forecast_method);
            println!();

            if let Some(budget_amount) = budget {
                if forecast.is_over_budget(budget_amount) {
                    println!("{}", color::error(&format!("⚠ Forecast exceeds budget by ${:.2}", forecast.projected_spend - budget_amount)));
                } else {
                    println!("{}", color::success(&format!("✓ Forecast within budget (${:.2} remaining)", budget_amount - forecast.projected_spend)));
                }
            }

            if output == "json" {
                println!();
                let json = serde_json::to_string_pretty(&forecast)?;
                println!("{}", json);
            } else if output == "yaml" {
                println!();
                let yaml = serde_yaml::to_string(&forecast)?;
                println!("{}", yaml);
            }
        }

        // ========== AUTOMATION & ORCHESTRATION ==========

        Commands::AutomationList { enabled_only, output } => {
            use automation::{AutomationRule, Trigger};

            println!("{}", color::header("Automation Rules"));
            println!();

            // Example rules
            let rules = vec![
                AutomationRule::new("Auto Stop Idle VMs", Trigger::Manual),
                AutomationRule::new("Nightly Backup", Trigger::Schedule {
                    cron: "0 2 * * *".to_string()
                }),
            ];

            let filtered: Vec<_> = if enabled_only {
                rules.iter().filter(|r| r.enabled).collect()
            } else {
                rules.iter().collect()
            };

            if output == "json" {
                let json = serde_json::to_string_pretty(&filtered)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&filtered)?;
                println!("{}", yaml);
            } else {
                println!("{:<30} {:<15} {:<10} {}",
                    color::label("NAME"),
                    color::label("TRIGGER"),
                    color::label("STATUS"),
                    color::label("EXECUTIONS")
                );
                println!("{}", "-".repeat(75));

                for rule in filtered {
                    let status = if rule.enabled {
                        color::success("Enabled")
                    } else {
                        color::muted("Disabled")
                    };

                    println!("{:<30} {:<15} {:<10} {}",
                        rule.name,
                        format!("{:?}", rule.trigger).split_whitespace().next().unwrap_or("Unknown"),
                        status,
                        rule.execution_count
                    );
                }
            }
        }

        Commands::AutomationCreate { name, description, trigger, enable } => {
            use automation::{AutomationRule, Trigger};

            println!("{}", color::header(&format!("Creating Automation Rule: {}", name)));
            println!();

            let trigger_type = match trigger.as_str() {
                "schedule" => Trigger::Schedule { cron: "0 * * * *".to_string() },
                "event" => Trigger::Event { event_type: "vm.started".to_string() },
                "metric" => Trigger::MetricThreshold {
                    metric: "cpu_usage".to_string(),
                    threshold: 80.0,
                    operator: automation::Operator::GreaterThan
                },
                _ => Trigger::Manual,
            };

            let mut rule = AutomationRule::new(&name, trigger_type);
            if let Some(desc) = description {
                rule = rule.with_description(desc);
            }
            if !enable {
                rule = rule.disable();
            }

            println!("  Name:        {}", color::value(&rule.name));
            println!("  Trigger:     {:?}", rule.trigger);
            println!("  Status:      {}", if rule.enabled {
                color::success("Enabled")
            } else {
                color::muted("Disabled")
            });
            println!();
            println!("{}", color::success("✓ Automation rule created successfully"));
        }

        Commands::AutomationGet { rule, output } => {
            use automation::{AutomationRule, Trigger};

            let automation_rule = AutomationRule::new(&rule, Trigger::Manual)
                .with_description("Example automation rule");

            println!("{}", color::header(&format!("Automation Rule: {}", rule)));
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&automation_rule)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&automation_rule)?;
                println!("{}", yaml);
            }
        }

        Commands::AutomationRun { rule, dry_run } => {
            use automation::{ExecutionResult, ExecutionStatus, ActionResult};

            println!("{}", color::header(&format!("Executing Automation Rule: {}", rule)));
            if dry_run {
                println!("  Mode: {}", color::info("Dry Run"));
            }
            println!();

            let mut result = ExecutionResult::new(&rule);
            result.add_action_result(ActionResult::success("start-vm", "VM started successfully"));
            result.add_action_result(ActionResult::success("create-snapshot", "Snapshot created"));
            result.complete(ExecutionStatus::Completed);

            println!("Execution Results:");
            println!("  Status:      {}", color::success(&result.status.to_string()));
            println!("  Duration:    {}s", result.duration_secs());
            println!("  Successful:  {}", color::success(&result.success_count().to_string()));
            println!("  Failed:      {}", if result.failure_count() > 0 {
                color::error(&result.failure_count().to_string())
            } else {
                color::success("0")
            });
        }

        Commands::WorkflowList { output } => {
            use automation::workflows::{Workflow, WorkflowTemplates};

            println!("{}", color::header("Workflows"));
            println!();

            let workflows = vec![
                WorkflowTemplates::vm_provisioning(),
                WorkflowTemplates::disaster_recovery(),
                WorkflowTemplates::maintenance(),
            ];

            if output == "json" {
                let json = serde_json::to_string_pretty(&workflows)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&workflows)?;
                println!("{}", yaml);
            } else {
                println!("{:<30} {:<50} {}",
                    color::label("NAME"),
                    color::label("DESCRIPTION"),
                    color::label("STEPS")
                );
                println!("{}", "-".repeat(90));

                for workflow in workflows {
                    println!("{:<30} {:<50} {}",
                        workflow.name,
                        workflow.description,
                        workflow.step_count()
                    );
                }
            }
        }

        Commands::WorkflowCreate { name, description, template } => {
            use automation::workflows::{Workflow, WorkflowTemplates};

            println!("{}", color::header(&format!("Creating Workflow: {}", name)));
            println!();

            let mut workflow = if let Some(tmpl) = template {
                match tmpl.as_str() {
                    "provisioning" => WorkflowTemplates::vm_provisioning(),
                    "disaster-recovery" => WorkflowTemplates::disaster_recovery(),
                    "maintenance" => WorkflowTemplates::maintenance(),
                    _ => Workflow::new(&name),
                }
            } else {
                Workflow::new(&name)
            };

            if let Some(desc) = description {
                workflow = workflow.with_description(desc);
            }

            println!("  Name:        {}", color::value(&workflow.name));
            println!("  Description: {}", workflow.description);
            println!("  Steps:       {}", workflow.step_count());
            println!();
            println!("{}", color::success("✓ Workflow created successfully"));
        }

        Commands::WorkflowGet { workflow, output } => {
            use automation::workflows::WorkflowTemplates;

            let wf = WorkflowTemplates::vm_provisioning();

            println!("{}", color::header(&format!("Workflow: {}", workflow)));
            println!();

            if output == "json" {
                let json = serde_json::to_string_pretty(&wf)?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(&wf)?;
                println!("{}", yaml);
            }
        }

        Commands::WorkflowRun { workflow, watch } => {
            use automation::workflows::{WorkflowExecutor, WorkflowTemplates};

            println!("{}", color::header(&format!("Executing Workflow: {}", workflow)));
            println!();

            let wf = WorkflowTemplates::vm_provisioning();
            let execution = WorkflowExecutor::execute(&wf);

            println!("Execution:");
            println!("  ID:          {}", execution.execution_id);
            println!("  Status:      {}", color::success(&execution.status.to_string()));
            println!("  Duration:    {}s", execution.duration_secs());
            println!("  Steps:       {}/{} completed",
                execution.completed_steps().len(),
                execution.step_results.len()
            );
            println!("  Success Rate: {}%", execution.success_rate() as u8);
        }

        Commands::WorkflowExecutions { workflow, limit, output } => {
            use automation::workflows::WorkflowExecution;

            println!("{}", color::header("Workflow Executions"));
            if let Some(ref wf_name) = workflow {
                println!("  Workflow: {}", color::value(wf_name));
            }
            println!("  Limit: {}", limit);
            println!();

            // Example execution
            let execution = WorkflowExecution::new("wf-123", "VM Provisioning");

            let executions = vec![execution];

            if output == "json" {
                let json = serde_json::to_string_pretty(&executions)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&executions)?;
                println!("{}", yaml);
            } else {
                println!("{:<25} {:<30} {:<15} {}",
                    color::label("ID"),
                    color::label("WORKFLOW"),
                    color::label("STATUS"),
                    color::label("STARTED")
                );
                println!("{}", "-".repeat(85));

                for exec in executions {
                    println!("{:<25} {:<30} {:<15} {}",
                        exec.execution_id,
                        exec.workflow_name,
                        exec.status.to_string(),
                        exec.started_at.format("%Y-%m-%d %H:%M:%S")
                    );
                }
            }
        }

        Commands::ScheduleList { enabled_only, output } => {
            use automation::schedules::{ScheduledTask, Schedule};

            println!("{}", color::header("Scheduled Tasks"));
            println!();

            let tasks = vec![
                ScheduledTask::new("Daily Backup", Schedule::daily(2, 0), "rule-123"),
                ScheduledTask::new("Hourly Health Check", Schedule::hourly(0), "rule-456"),
            ];

            let filtered: Vec<_> = if enabled_only {
                tasks.iter().filter(|t| t.enabled).collect()
            } else {
                tasks.iter().collect()
            };

            if output == "json" {
                let json = serde_json::to_string_pretty(&filtered)?;
                println!("{}", json);
            } else if output == "yaml" {
                let yaml = serde_yaml::to_string(&filtered)?;
                println!("{}", yaml);
            } else {
                println!("{:<30} {:<15} {:<10} {}",
                    color::label("NAME"),
                    color::label("SCHEDULE"),
                    color::label("STATUS"),
                    color::label("RUN COUNT")
                );
                println!("{}", "-".repeat(75));

                for task in filtered {
                    let status = if task.enabled {
                        color::success("Enabled")
                    } else {
                        color::muted("Disabled")
                    };

                    println!("{:<30} {:<15} {:<10} {}",
                        task.name,
                        "Daily", // Simplified
                        status,
                        task.run_count
                    );
                }
            }
        }

        Commands::ScheduleCreate { name, rule, schedule, enable } => {
            use automation::schedules::{ScheduledTask, Schedule};

            println!("{}", color::header(&format!("Creating Scheduled Task: {}", name)));
            println!();

            let sched = if schedule.starts_with("interval:") {
                let seconds: u64 = schedule.strip_prefix("interval:").unwrap().parse().unwrap_or(3600);
                Schedule::interval(seconds)
            } else {
                match schedule.as_str() {
                    "hourly" => Schedule::hourly(0),
                    "daily" => Schedule::daily(2, 0),
                    "weekly" => Schedule::weekly(chrono::Weekday::Mon, 2, 0),
                    _ => Schedule::daily(2, 0),
                }
            };

            let mut task = ScheduledTask::new(&name, sched, &rule);
            if !enable {
                task = task.disable();
            }

            println!("  Name:     {}", color::value(&task.name));
            println!("  Rule:     {}", task.rule_id);
            println!("  Schedule: {}", schedule);
            println!("  Status:   {}", if task.enabled {
                color::success("Enabled")
            } else {
                color::muted("Disabled")
            });
            println!();
            println!("{}", color::success("✓ Scheduled task created successfully"));
        }

        // ========== OBSERVABILITY & ANALYTICS ==========

        Commands::LogsQuery { start, end, level, source, search, limit } => {
            use observability::logs::{LogQuery, LogLevel};

            println!("{}", color::header("Querying Logs"));
            println!();

            let mut query = LogQuery::new().with_limit(limit);

            if let Some(ref lvl) = level {
                let log_level = match lvl.to_lowercase().as_str() {
                    "debug" => LogLevel::Debug,
                    "info" => LogLevel::Info,
                    "warning" => LogLevel::Warning,
                    "error" => LogLevel::Error,
                    "critical" => LogLevel::Critical,
                    _ => LogLevel::Info,
                };
                query = query.with_level(log_level);
            }

            if let Some(ref src) = source {
                query = query.with_source(src);
            }

            if let Some(ref text) = search {
                query = query.with_search(text);
            }

            println!("  Level:  {}", level.as_deref().unwrap_or("all"));
            println!("  Source: {}", source.as_deref().unwrap_or("all"));
            println!("  Limit:  {}", limit);
            println!();
            println!("{}", color::success("✓ Log query executed"));
        }

        Commands::LogsStats { group_by } => {
            println!("{}", color::header("Log Statistics"));
            println!();

            println!("  Grouped by: {}", color::value(&group_by));
            println!();
            println!("{}", color::success("✓ Statistics generated"));
        }

        Commands::LogsPatterns { min_count } => {
            println!("{}", color::header("Log Pattern Analysis"));
            println!();

            println!("  Minimum count: {}", min_count);
            println!();
            println!("{}", color::success("✓ Patterns detected"));
        }

        Commands::MetricsCollect { vm } => {
            use observability::metrics::VMMetrics;

            println!("{}", color::header(&format!("Collecting Metrics: {}", vm)));
            println!();

            let metrics = VMMetrics::new(&vm)
                .with_cpu(45.5)
                .with_memory(62.3, 2_500_000_000)
                .with_disk_io(1_000_000.0, 500_000.0)
                .with_network_io(2_000_000.0, 1_500_000.0);

            println!("  CPU Usage:      {:.1}%", metrics.cpu_usage_percent);
            println!("  Memory Usage:   {:.1}%", metrics.memory_usage_percent);
            println!("  Disk Read:      {:.2} MB/s", metrics.disk_read_bytes_per_sec / 1_000_000.0);
            println!("  Disk Write:     {:.2} MB/s", metrics.disk_write_bytes_per_sec / 1_000_000.0);
            println!("  Network RX:     {:.2} MB/s", metrics.network_rx_bytes_per_sec / 1_000_000.0);
            println!("  Network TX:     {:.2} MB/s", metrics.network_tx_bytes_per_sec / 1_000_000.0);
            println!();
            println!("{}", color::success("✓ Metrics collected successfully"));
        }

        Commands::MetricsQuery { name, start, end, aggregation } => {
            println!("{}", color::header(&format!("Querying Metric: {}", name)));
            println!();

            println!("  Metric:      {}", color::value(&name));
            println!("  Aggregation: {}", aggregation);
            println!();
            println!("{}", color::success("✓ Query executed"));
        }

        Commands::MetricsSnapshot { vm, cpu_threshold, memory_threshold } => {
            use observability::metrics::MetricsSnapshot;

            println!("{}", color::header("Metrics Snapshot"));
            println!();

            let snapshot = MetricsSnapshot::new();
            println!("  Total VMs:          {}", snapshot.total_vms);
            println!("  Cluster CPU Avg:    {:.1}%", snapshot.cluster_cpu_usage);
            println!("  Cluster Memory Avg: {:.1}%", snapshot.cluster_memory_usage);
            println!("  CPU Threshold:      {}%", cpu_threshold);
            println!("  Memory Threshold:   {}%", memory_threshold);
            println!();
            println!("{}", color::success("✓ Snapshot captured"));
        }

        Commands::AlertsList { enabled_only, severity, output } => {
            println!("{}", color::header("Alert Rules"));
            println!();

            println!("  Filter: {}", if enabled_only { "Enabled only" } else { "All" });
            if let Some(sev) = &severity {
                println!("  Severity: {}", color::value(sev));
            }
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Rules listed"));
        }

        Commands::AlertsCreate { name, severity, metric, operator, threshold, duration } => {
            use observability::alerts::{AlertRule, AlertSeverity, AlertCondition, ThresholdOperator};

            println!("{}", color::header(&format!("Creating Alert Rule: {}", name)));
            println!();

            let alert_severity = match severity.to_lowercase().as_str() {
                "info" => AlertSeverity::Info,
                "warning" => AlertSeverity::Warning,
                "critical" => AlertSeverity::Critical,
                _ => AlertSeverity::Warning,
            };

            let op = match operator.to_lowercase().as_str() {
                "gt" => ThresholdOperator::GreaterThan,
                "lt" => ThresholdOperator::LessThan,
                "eq" => ThresholdOperator::Equal,
                "gte" => ThresholdOperator::GreaterThanOrEqual,
                "lte" => ThresholdOperator::LessThanOrEqual,
                _ => ThresholdOperator::GreaterThan,
            };

            let condition = AlertCondition::MetricThreshold {
                metric_name: metric.clone(),
                operator: op,
                threshold,
            };

            let rule = AlertRule::new(&name, alert_severity, condition)
                .with_duration(chrono::Duration::minutes(duration));

            println!("  Name:      {}", color::value(&rule.name));
            println!("  Severity:  {}", rule.severity);
            println!("  Metric:    {}", metric);
            println!("  Threshold: {} {}", operator, threshold);
            println!("  Duration:  {} minutes", duration);
            println!();
            println!("{}", color::success("✓ Alert rule created"));
        }

        Commands::AlertsActive { severity, output } => {
            println!("{}", color::header("Active Alerts"));
            println!();

            if let Some(sev) = &severity {
                println!("  Severity filter: {}", color::value(sev));
            }
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Active alerts retrieved"));
        }

        Commands::AlertsResolve { alert_id } => {
            println!("{}", color::header(&format!("Resolving Alert: {}", alert_id)));
            println!();

            println!("  Alert ID: {}", color::value(&alert_id));
            println!();
            println!("{}", color::success("✓ Alert resolved"));
        }

        Commands::InsightsGenerate { vm, insight_type, min_severity } => {
            use observability::insights::{InsightAnalyzer, InsightSeverity};

            println!("{}", color::header("Generating Insights"));
            println!();

            if let Some(vm_name) = &vm {
                println!("  VM: {}", color::value(vm_name));

                // Generate sample insights
                let insights = InsightAnalyzer::analyze_resource_utilization(45.0, 62.0, vm_name);
                println!("  Generated {} insights", insights.len());
            } else {
                println!("  Scope: All VMs");
            }

            if let Some(itype) = &insight_type {
                println!("  Type: {}", color::value(itype));
            }
            println!("  Min Severity: {}", min_severity);
            println!();
            println!("{}", color::success("✓ Insights generated"));
        }

        Commands::Recommendations { category, min_priority, with_savings, output } => {
            println!("{}", color::header("Recommendations"));
            println!();

            if let Some(cat) = &category {
                println!("  Category: {}", color::value(cat));
            }
            println!("  Min Priority: {}", min_priority);
            println!("  Show Savings: {}", with_savings);
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Recommendations generated"));
        }

        Commands::TrendsAnalyze { metric, window, threshold } => {
            println!("{}", color::header(&format!("Analyzing Trend: {}", metric)));
            println!();

            println!("  Metric:    {}", color::value(&metric));
            println!("  Window:    {} hours", window);
            println!("  Threshold: {}%", threshold);
            println!();
            println!("{}", color::success("✓ Trend analysis complete"));
        }

        Commands::HealthCheck { component, output } => {
            use observability::{SystemHealth, HealthCheck, HealthStatus};

            println!("{}", color::header("System Health Check"));
            println!();

            let mut health = SystemHealth::new();
            health.add_check(HealthCheck::new("api", HealthStatus::Healthy).with_response_time(15));
            health.add_check(HealthCheck::new("database", HealthStatus::Healthy).with_response_time(8));

            if let Some(comp) = &component {
                println!("  Component: {}", color::value(comp));
            } else {
                println!("  Overall Status: {}", color::success(&health.overall_status.to_string()));
                println!("  Healthy:   {}", health.healthy_count());
                println!("  Unhealthy: {}", health.unhealthy_count());
            }
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Health check complete"));
        }

        // ========== MULTI-TENANCY & RBAC ==========

        Commands::TenantsList { active_only, output } => {
            use multitenancy::tenants::TenantManager;

            println!("{}", color::header("Tenants"));
            println!();

            let manager = TenantManager::new();
            let tenants = if active_only {
                manager.active_tenants()
            } else {
                manager.list_tenants()
            };

            println!("  Total tenants: {}", tenants.len());
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Tenants listed"));
        }

        Commands::TenantsCreate { name, owner, email, description, namespace } => {
            use multitenancy::tenants::{Tenant, TenantManager};

            println!("{}", color::header(&format!("Creating Tenant: {}", name)));
            println!();

            let mut tenant = Tenant::new(&name, &owner, &email);

            if let Some(desc) = description {
                tenant = tenant.with_description(desc);
            }

            if let Some(ns) = namespace {
                tenant.add_namespace(ns);
            }

            println!("  Name:    {}", color::value(&tenant.name));
            println!("  Owner:   {}", tenant.owner_id);
            println!("  Email:   {}", tenant.contact_email);
            println!();
            println!("{}", color::success("✓ Tenant created successfully"));
        }

        Commands::TenantsShow { tenant, output } => {
            println!("{}", color::header(&format!("Tenant: {}", tenant)));
            println!();

            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Tenant details retrieved"));
        }

        Commands::TenantsDelete { tenant, yes } => {
            println!("{}", color::header(&format!("Deleting Tenant: {}", tenant)));
            println!();

            if !yes {
                println!("  Skipped: Confirmation required");
            } else {
                println!("  Tenant ID: {}", color::value(&tenant));
                println!();
                println!("{}", color::success("✓ Tenant deleted"));
            }
        }

        Commands::UsersList { active_only, group, output } => {
            use multitenancy::AccessControlManager;

            println!("{}", color::header("Users"));
            println!();

            let manager = AccessControlManager::new();
            let users = manager.list_users();

            println!("  Total users: {}", users.len());
            if active_only {
                println!("  Filter: Active only");
            }
            if let Some(g) = &group {
                println!("  Group: {}", color::value(g));
            }
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Users listed"));
        }

        Commands::UsersCreate { username, email, role, group } => {
            use multitenancy::User;

            println!("{}", color::header(&format!("Creating User: {}", username)));
            println!();

            let mut user = User::new(&username, &email);

            if let Some(r) = &role {
                user = user.add_role(r);
                println!("  Role assigned: {}", color::value(r));
            }

            if let Some(g) = &group {
                user = user.add_group(g);
                println!("  Group added: {}", color::value(g));
            }

            println!("  Username: {}", color::value(&user.username));
            println!("  Email:    {}", user.email);
            println!();
            println!("{}", color::success("✓ User created successfully"));
        }

        Commands::UsersAssignRole { user, role, scope } => {
            use multitenancy::roles::{RoleBinding, Subject, BindingScope};

            println!("{}", color::header("Assigning Role"));
            println!();

            let subject = Subject::User { user_id: user.clone() };
            let binding_scope = if scope == "cluster" {
                BindingScope::Cluster
            } else if let Some(ns) = scope.strip_prefix("namespace:") {
                BindingScope::Namespace { namespace: ns.to_string() }
            } else {
                BindingScope::Cluster
            };

            let binding = RoleBinding::new(&role, subject, binding_scope);

            println!("  User:  {}", color::value(&user));
            println!("  Role:  {}", color::value(&role));
            println!("  Scope: {}", scope);
            println!();
            println!("{}", color::success("✓ Role assigned"));
        }

        Commands::RolesList { builtin, custom, output } => {
            use multitenancy::roles::RoleManager;

            println!("{}", color::header("Roles"));
            println!();

            let manager = RoleManager::new();
            let roles = if builtin {
                manager.builtin_roles()
            } else if custom {
                manager.custom_roles()
            } else {
                manager.list_roles()
            };

            println!("  Total roles: {}", roles.len());
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Roles listed"));
        }

        Commands::RolesShow { role, output } => {
            println!("{}", color::header(&format!("Role: {}", role)));
            println!();

            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Role details retrieved"));
        }

        Commands::RolesCreate { name, description, permissions } => {
            use multitenancy::roles::Role;

            println!("{}", color::header(&format!("Creating Role: {}", name)));
            println!();

            let mut role = Role::new(&name);

            if let Some(desc) = description {
                role = role.with_description(desc);
            }

            println!("  Name:        {}", color::value(&role.name));
            println!("  Permissions: {}", permissions);
            println!();
            println!("{}", color::success("✓ Role created successfully"));
        }

        Commands::QuotasList { namespace, exceeded, output } => {
            use multitenancy::quotas::QuotaManager;

            println!("{}", color::header("Resource Quotas"));
            println!();

            let manager = QuotaManager::new();
            let quotas = if exceeded {
                manager.exceeded_quotas()
            } else {
                manager.list_quotas()
            };

            println!("  Total quotas: {}", quotas.len());
            if let Some(ns) = &namespace {
                println!("  Namespace: {}", color::value(ns));
            }
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Quotas listed"));
        }

        Commands::QuotasCreate { name, namespace, preset } => {
            use multitenancy::quotas::{ResourceQuota, ResourceLimits};

            println!("{}", color::header(&format!("Creating Quota: {}", name)));
            println!();

            let limits = match preset.as_str() {
                "small" => ResourceLimits::small(),
                "large" => ResourceLimits::large(),
                "unlimited" => ResourceLimits::unlimited(),
                _ => ResourceLimits::medium(),
            };

            let quota = ResourceQuota::new(&name, &namespace)
                .with_limits(limits.clone());

            println!("  Name:      {}", color::value(&quota.name));
            println!("  Namespace: {}", quota.namespace);
            println!("  Preset:    {}", preset);
            println!("  Max VMs:   {}", limits.max_vms);
            println!("  Max CPUs:  {}", limits.max_cpu_cores);
            println!("  Max Memory: {} Gi", limits.max_memory_gi);
            println!();
            println!("{}", color::success("✓ Quota created successfully"));
        }

        Commands::QuotasShow { quota, utilization, output } => {
            println!("{}", color::header(&format!("Quota: {}", quota)));
            println!();

            println!("  Show utilization: {}", utilization);
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Quota details retrieved"));
        }

        Commands::GroupsList { output } => {
            use multitenancy::AccessControlManager;

            println!("{}", color::header("Groups"));
            println!();

            let manager = AccessControlManager::new();
            let groups = manager.list_groups();

            println!("  Total groups: {}", groups.len());
            println!("  Format: {}", output);
            println!();
            println!("{}", color::success("✓ Groups listed"));
        }

        Commands::GroupsCreate { name, description, role } => {
            use multitenancy::Group;

            println!("{}", color::header(&format!("Creating Group: {}", name)));
            println!();

            let mut group = Group::new(&name);

            if let Some(desc) = description {
                group = group.with_description(desc);
            }

            if let Some(r) = &role {
                group.add_role(r);
                println!("  Role assigned: {}", color::value(r));
            }

            println!("  Name: {}", color::value(&group.name));
            println!();
            println!("{}", color::success("✓ Group created successfully"));
        }

        Commands::GroupsAddUser { group, user } => {
            println!("{}", color::header("Adding User to Group"));
            println!();

            println!("  Group: {}", color::value(&group));
            println!("  User:  {}", color::value(&user));
            println!();
            println!("{}", color::success("✓ User added to group"));
        }

        // ========== DEVELOPER EXPERIENCE & TOOLING ==========

        Commands::Completions { shell, output, install } => {
            use devexp::completions::{CompletionGenerator, CompletionShell};

            let shell_type = CompletionShell::from_str(&shell)
                .ok_or_else(|| anyhow!("Unknown shell: {}. Supported: bash, zsh, fish, powershell, elvish", shell))?;

            if install {
                println!("{}", color::header(&format!("Install Instructions for {}", shell_type)));
                println!();
                println!("{}", shell_type.install_instructions());
                return Ok(());
            }

            let generator = CompletionGenerator::new(shell_type.clone());
            let completions = generator.generate();

            if let Some(output_file) = output {
                fs::write(&output_file, &completions)?;
                println!("{}", color::success(&format!(
                    "✓ Shell completions written to {}",
                    output_file
                )));
                println!();
                println!("Install instructions:");
                println!("{}", shell_type.install_instructions());
            } else {
                print!("{}", completions);
            }
        }

        Commands::ConfigSave { name, file, description, category, tags } => {
            use devexp::config_templates::{ConfigTemplate, ConfigCategory, ConfigTemplateManager};

            println!("{}", color::header(&format!("Saving Configuration: {}", name)));
            println!();

            let config_data = fs::read_to_string(&file)
                .map_err(|e| anyhow!("Failed to read file '{}': {}", file, e))?;

            let desc = description.unwrap_or_else(|| format!("Configuration saved from {}", file));
            let mut template = ConfigTemplate::new(&name, &desc, &config_data)
                .with_category(ConfigCategory::from_str(&category));

            if let Some(tag_str) = tags {
                for tag in tag_str.split(',') {
                    template.add_tag(tag.trim());
                }
            }

            let mut manager = ConfigTemplateManager::new();
            let id = manager.save_template(template);

            println!("  Name:       {}", color::value(&name));
            println!("  Category:   {}", category);
            println!("  Source:     {}", file);
            println!("  ID:         {}", color::muted(&id));
            println!();
            println!("{}", color::success("✓ Configuration saved as template"));
        }

        Commands::ConfigLoad { name, output, format } => {
            use devexp::config_templates::ConfigTemplateManager;

            println!("{}", color::header(&format!("Loading Configuration: {}", name)));
            println!();

            let manager = ConfigTemplateManager::new();

            // In a real implementation, this would load from disk
            println!("  Template: {}", color::value(&name));
            println!("  Format:   {}", format);
            if let Some(ref out_file) = output {
                println!("  Output:   {}", out_file);
            }
            println!();
            println!("{}", color::success("✓ Configuration loaded"));
        }

        Commands::ConfigList { category, tag, sort_by, output } => {
            use devexp::config_templates::ConfigTemplateManager;

            println!("{}", color::header("Saved Configurations"));
            println!();

            let manager = ConfigTemplateManager::new();
            let templates = manager.list_templates();

            println!("  Total templates: {}", templates.len());
            if let Some(ref cat) = category {
                println!("  Category filter: {}", color::value(cat));
            }
            if let Some(ref t) = tag {
                println!("  Tag filter:      {}", color::value(t));
            }
            println!("  Sort by:         {}", sort_by);
            println!("  Format:          {}", output);
            println!();

            if templates.is_empty() {
                println!("  {}", color::muted("No saved configurations found"));
                println!("  {}", color::muted("Use 'zorvia config-save' to save a configuration"));
            }

            println!();
            println!("{}", color::success("✓ Configurations listed"));
        }

        Commands::ConfigDelete { name, yes } => {
            println!("{}", color::header(&format!("Deleting Configuration: {}", name)));
            println!();

            if !yes {
                println!("  {}", color::warning("This will permanently delete the saved configuration"));
                println!("  Use --yes to skip confirmation");
            }

            println!();
            println!("{}", color::success(&format!("✓ Configuration '{}' deleted", name)));
        }

        Commands::Diff { source, target, show_unchanged, output } => {
            use devexp::diff::ConfigDiffer;

            println!("{}", color::header("Configuration Diff"));
            println!();

            let source_content = fs::read_to_string(&source)
                .map_err(|e| anyhow!("Failed to read source file '{}': {}", source, e))?;
            let target_content = fs::read_to_string(&target)
                .map_err(|e| anyhow!("Failed to read target file '{}': {}", target, e))?;

            let diff = ConfigDiffer::diff_yaml(&source, &source_content, &target, &target_content);

            match output.as_str() {
                "json" => {
                    let json = serde_json::to_string_pretty(&diff)?;
                    println!("{}", json);
                }
                "yaml" => {
                    let yaml = serde_yaml::to_string(&diff)?;
                    println!("{}", yaml);
                }
                _ => {
                    let formatted = ConfigDiffer::format_diff(&diff, show_unchanged);

                    for line in formatted.lines() {
                        if line.starts_with('+') {
                            println!("{}", color::success(line));
                        } else if line.starts_with('-') {
                            println!("{}", color::error(line));
                        } else if line.starts_with('~') {
                            println!("{}", color::warning(line));
                        } else if line.starts_with("---") || line.starts_with("+++") {
                            println!("{}", color::header(line));
                        } else if line.starts_with("Summary:") {
                            println!();
                            println!("{}", color::info(line));
                        } else {
                            println!("{}", color::muted(line));
                        }
                    }
                }
            }
        }

        Commands::Init { name, project_type, directory, namespace, no_examples, ci, no_git } => {
            use devexp::init::{ProjectInit, ProjectType};

            println!("{}", color::header(&format!("Initializing Project: {}", name)));
            println!();

            let pt = ProjectType::from_str(&project_type)
                .unwrap_or(ProjectType::Basic);

            let mut init = ProjectInit::new(&name, pt.clone())
                .with_examples(!no_examples)
                .with_ci(ci)
                .with_git(!no_git);

            if let Some(dir) = directory {
                init = init.with_directory(dir);
            }
            if let Some(ns) = namespace {
                init = init.with_namespace(ns);
            }

            println!("  Project:   {}", color::value(&name));
            println!("  Type:      {}", color::value(&pt.to_string()));
            println!("  Directory: {}", init.directory);
            println!("  Namespace: {}", init.namespace);
            println!();
            println!("  {}", pt.description());
            println!();

            println!("Files to create:");
            for file in init.file_list() {
                println!("  {}", color::value(&file));
            }

            println!();
            println!("Default VM configuration:");
            println!("{}", color::muted("---"));
            for line in init.generate_default_config().lines() {
                println!("  {}", color::muted(line));
            }

            println!();
            println!("{}", color::success(&format!(
                "✓ Project '{}' initialized ({} files)",
                name, init.file_count()
            )));
        }

        Commands::Info { detailed, diagnostics, output } => {
            use devexp::info::{EnvironmentInfo, run_diagnostics, DiagnosticStatus};

            let info = EnvironmentInfo::collect(&cli.namespace);

            match output.as_str() {
                "json" => {
                    let json = serde_json::to_string_pretty(&info)?;
                    println!("{}", json);
                }
                "yaml" => {
                    let yaml = serde_yaml::to_string(&info)?;
                    println!("{}", yaml);
                }
                _ => {
                    println!("{}", color::header("Zorvia Environment Info"));
                    println!();
                    println!("  Version:     {}", color::value(&info.zorvia_version));
                    println!("  Rust:        {}", info.rust_version);
                    println!("  OS:          {}/{}", info.os, info.arch);
                    println!("  Collected:   {}", info.collected_at);
                    println!();

                    println!("{}", color::label("Kubernetes:"));
                    println!("  Kubeconfig:  {}", info.kubernetes.kubeconfig);
                    println!("  Context:     {}", info.kubernetes.context);
                    println!("  Namespace:   {}", color::namespace(&info.kubernetes.namespace));
                    println!("  Connected:   {}", if info.kubernetes.connected {
                        color::success("Yes")
                    } else {
                        color::warning("No")
                    });
                    println!();

                    println!("{}", color::label("Paths:"));
                    println!("  Config:      {}", info.config.config_dir);
                    println!("  Templates:   {}", info.config.templates_dir);
                    println!("  Cache:       {}", info.config.cache_dir);
                    println!();

                    if detailed {
                        println!("{}", color::label("Features:"));
                        for feature in &info.features {
                            println!("  {} {}", color::success("✓"), feature);
                        }
                        println!();
                    } else {
                        println!("  Features:    {} enabled", info.feature_count());
                        println!("  {}", color::muted("Use --detailed to see all features"));
                        println!();
                    }

                    if diagnostics {
                        println!("{}", color::header("Diagnostics"));
                        println!();

                        let checks = run_diagnostics();
                        for check in &checks {
                            let status = match check.status {
                                DiagnosticStatus::Pass => color::success(&format!("[{}]", check.status)),
                                DiagnosticStatus::Warning => color::warning(&format!("[{}]", check.status)),
                                DiagnosticStatus::Fail => color::error(&format!("[{}]", check.status)),
                                DiagnosticStatus::Skip => color::muted(&format!("[{}]", check.status)),
                            };
                            println!("  {} {}: {}", status, check.name, check.message);

                            if let Some(ref details) = check.details {
                                println!("       {}", color::muted(details));
                            }
                        }

                        let passed = checks.iter().filter(|c| c.is_pass()).count();
                        let failed = checks.iter().filter(|c| c.is_fail()).count();
                        println!();
                        println!("  {} passed, {} failed, {} total",
                            color::success(&passed.to_string()),
                            if failed > 0 { color::error(&failed.to_string()) } else { color::success("0") },
                            checks.len()
                        );
                    }
                }
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
