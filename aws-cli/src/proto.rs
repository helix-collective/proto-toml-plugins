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
    let pkg_path = install_dir.join("AWSCLIV2.pkg");
    let expanded_path = install_dir.join("_expanded");

    // Download the .pkg
    exec_command!(raw, "curl", [
        "-fSL", "-o",
        &pkg_path.to_string(),
        &pkg_url
    ])?;

    // Extract the .pkg payload
    exec_command!(raw, "pkgutil", [
        "--expand-full",
        &pkg_path.to_string(),
        &expanded_path.to_string()
    ])?;

    // Move the aws-cli payload into place
    exec_command!(raw, "mv", [
        &expanded_path.join("aws-cli").join("Payload").join("aws-cli").to_string(),
        &install_dir.join("aws-cli").to_string()
    ])?;

    // Clean up
    exec_command!(raw, "rm", [
        "-rf",
        &pkg_path.to_string(),
        &expanded_path.to_string()
    ])?;

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
