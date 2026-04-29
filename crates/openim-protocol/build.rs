use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let protocol_dir = env::var_os("OPENIM_PROTOCOL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../../protocol"));

    println!("cargo:rerun-if-env-changed=OPENIM_PROTOCOL_DIR");
    println!("cargo:rerun-if-changed={}", protocol_dir.display());

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    env::set_var("PROTOC", protoc);

    let protos = [
        "wrapperspb/wrapperspb.proto",
        "sdkws/sdkws.proto",
        "conversation/conversation.proto",
        "msg/msg.proto",
    ]
    .map(|path| protocol_dir.join(path));

    let mut config = prost_build::Config::new();
    config.disable_comments(&["."]);
    config.compile_protos(&protos, &[protocol_dir])?;

    Ok(())
}
