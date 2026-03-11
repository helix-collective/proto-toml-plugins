use extism_pdk::*;
use proto_pdk::*;
use std::collections::HashMap;

#[host_fn]
extern "ExtismHost" {
    fn exec_command(input: Json<ExecCommandInput>) -> Json<ExecCommandOutput>;
}

#[plugin_fn]
pub fn register_tool(_: ()) -> FnResult<Json<RegisterToolOutput>> {
    Ok(Json(RegisterToolOutput {
        name: "aws-cli".into(),
        type_of: PluginType::CommandLine,
        ..RegisterToolOutput::default()
    }))
}

#[plugin_fn]
pub fn load_versions(_: Json<LoadVersionsInput>) -> FnResult<Json<LoadVersionsOutput>> {
    let tags = load_git_tags("https://github.com/aws/aws-cli")?;

    let versions = tags
        .into_iter()
        .filter(|tag| tag.starts_with('2'))
        .collect::<Vec<_>>();

    Ok(Json(LoadVersionsOutput::from(versions)?))
}

#[plugin_fn]
pub fn download_prebuilt(
    Json(input): Json<DownloadPrebuiltInput>,
) -> FnResult<Json<DownloadPrebuiltOutput>> {
    let version = &input.context.version;
    let env = get_host_environment()?;

    check_supported_os_and_arch(
        "aws-cli",
        &env,
        permutations![
            HostOS::Linux => [HostArch::X64, HostArch::Arm64],
            HostOS::MacOS => [HostArch::X64, HostArch::Arm64],
        ],
    )?;

    let (download_url, download_name) = match env.os {
        HostOS::Linux => {
            let arch = match env.arch {
                HostArch::X64 => "x86_64",
                HostArch::Arm64 => "aarch64",
                _ => unreachable!(),
            };
            let filename = format!("awscli-exe-linux-{arch}-{version}.zip");
            let url = format!("https://awscli.amazonaws.com/{filename}");
            (url, filename)
        }
        HostOS::MacOS => {
            let filename = format!("AWSCLIV2-{version}.pkg");
            let url = format!("https://awscli.amazonaws.com/{filename}");
            (url, filename)
        }
        _ => unreachable!(),
    };

    Ok(Json(DownloadPrebuiltOutput {
        download_url,
        download_name: Some(download_name),
        ..DownloadPrebuiltOutput::default()
    }))
}

fn run_command(cmd: &str, args: &[&str]) -> Result<(), Error> {
    let result = exec_command!(raw, cmd, args);
    match result {
        Ok(output) => {
            if output.0.exit_code != 0 {
                return Err(Error::msg(format!(
                    "{} failed (exit {}): {}",
                    cmd, output.0.exit_code, output.0.stderr
                )));
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[plugin_fn]
pub fn native_install(
    Json(input): Json<NativeInstallInput>,
) -> FnResult<Json<NativeInstallOutput>> {
    let env = get_host_environment()?;

    // Linux: let proto handle zip extraction natively
    if env.os != HostOS::MacOS {
        return Ok(Json(NativeInstallOutput {
            skip_install: true,
            ..NativeInstallOutput::default()
        }));
    }

    // macOS: extract .pkg using pkgutil
    let version = &input.context.version;
    let install_dir = &input.context.tool_dir;
    let pkg_url = format!("https://awscli.amazonaws.com/AWSCLIV2-{version}.pkg");

    // Use /tmp for download and extraction to avoid virtual path issues
    let pkg_path = "/tmp/proto-awscli.pkg";
    let expanded_path = "/tmp/proto-awscli-expanded";
    let install_dir_str = install_dir
        .real_path()
        .expect("install_dir real path")
        .to_string_lossy()
        .to_string();

    // Download the .pkg
    run_command("curl", &["-fSL", "-o", pkg_path, &pkg_url])?;

    // Extract the .pkg payload
    run_command("pkgutil", &["--expand-full", pkg_path, expanded_path])?;

    // Copy the aws-cli payload into the install directory
    let payload_src = format!("{}/aws-cli.pkg/Payload/aws-cli", expanded_path);
    let payload_dst = format!("{}/aws-cli", install_dir_str);
    run_command("cp", &["-R", &payload_src, &payload_dst])?;

    // Clean up
    run_command("rm", &["-rf", pkg_path, expanded_path])?;

    Ok(Json(NativeInstallOutput {
        installed: true,
        ..NativeInstallOutput::default()
    }))
}

#[plugin_fn]
pub fn locate_executables(
    _: Json<LocateExecutablesInput>,
) -> FnResult<Json<LocateExecutablesOutput>> {
    let env = get_host_environment()?;

    let exe_path: String = match env.os {
        HostOS::MacOS => "aws-cli/aws".into(),
        _ => "aws/dist/aws".into(),
    };

    Ok(Json(LocateExecutablesOutput {
        exes: HashMap::from_iter([(
            "aws".into(),
            ExecutableConfig::new_primary(exe_path),
        )]),
        ..LocateExecutablesOutput::default()
    }))
}
