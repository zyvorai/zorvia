// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Off-cluster backup Job entrypoint: streams the VM disk images mounted
//! read-only under /disks/ to S3-compatible storage (see
//! `zorvia::backup::agent`). Configuration arrives via environment
//! variables set on the Job by `src/kube/backup_job.rs`; credentials and the
//! encryption key come from Kubernetes Secrets, never from arguments.
//!
//! stdout is a stream of JSON lines. Progress lines look like
//! `{"progress":42,"phase":"uploading root"}`; the last line is the result:
//! `{"success":true,...}` or `{"success":false,"error":"..."}`. Zorvia reads
//! them back from the Job pod's logs. Exits 0 on success, 1 on failure.

use std::process::ExitCode;
use zorvia::backup::agent::{run, AgentConfig};
use zorvia::backup::s3::S3Client;

fn line(v: serde_json::Value) {
    println!("{v}");
}

#[tokio::main]
async fn main() -> ExitCode {
    let cfg = match AgentConfig::from_env(|k| std::env::var(k).ok()) {
        Ok(c) => c,
        Err(e) => {
            line(serde_json::json!({"success": false, "error": format!("{e:#}")}));
            return ExitCode::FAILURE;
        }
    };
    let client = match S3Client::new(cfg.s3.clone()) {
        Ok(c) => c,
        Err(e) => {
            line(serde_json::json!({"success": false, "error": format!("{e:#}")}));
            return ExitCode::FAILURE;
        }
    };
    let progress =
        |pct: u8, phase: &str| line(serde_json::json!({"progress": pct, "phase": phase}));

    match run(&cfg, &client, &progress).await {
        Ok(result) => {
            let mut v = serde_json::to_value(&result).unwrap_or_default();
            v["success"] = serde_json::json!(true);
            line(v);
            ExitCode::SUCCESS
        }
        Err(e) => {
            line(serde_json::json!({"success": false, "error": format!("{e:#}")}));
            ExitCode::FAILURE
        }
    }
}
