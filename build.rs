#[cfg(feature = "vendored")]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let artifacts = lua_src::Build::new().build(lua_src::Lua54);
    artifacts.print_cargo_metadata();
}

#[cfg(not(feature = "vendored"))]
fn main() {
    let family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
    match family.as_str() {
	"windows" => println!("cargo:rustc-link-lib=lua54.dll"),
	"unix" => println!("cargo:rustc-link-lib=lua5.4"),
	_ => ()
    }
}
