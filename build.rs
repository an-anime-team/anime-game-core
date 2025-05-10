fn main() {
    #[cfg(feature = "sophon")]
    protobuf_codegen::Codegen::new()
        .cargo_out_dir("protos")
        .include("src")
        .input("src/sophon/protos/SophonManifest.proto")
        .input("src/sophon/protos/SophonPatch.proto")
        .run_from_script();
}
