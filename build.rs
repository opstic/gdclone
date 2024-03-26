use std::collections::HashMap;
use std::env;
use std::fmt::{Display, Write};
use std::fs::File;
use std::io::{BufWriter, Read, Write as IoWrite};
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

#[derive(Deserialize)]
struct ObjectData {
    texture: Option<String>,
    default_z_layer: Option<i8>,
    default_z_order: Option<i16>,
    default_base_color_channel: Option<u64>,
    default_detail_color_channel: Option<u64>,
    color_type: Option<String>,
    swap_base_detail: Option<bool>,
    opacity: Option<f32>,
    hitbox: Option<Hitbox>,
    children: Option<Vec<Child>>,
}

#[derive(Deserialize)]
struct Child {
    texture: String,
    x: f32,
    y: f32,
    z: i16,
    rot: f32,
    anchor_x: f32,
    anchor_y: f32,
    scale_x: f32,
    scale_y: f32,
    flip_x: bool,
    flip_y: bool,
    color_type: Option<String>,
    opacity: Option<f32>,
    children: Option<Vec<Child>>,
}

#[derive(Deserialize)]
struct Hitbox {
    r#type: String,
    x: Option<f32>,
    y: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    radius: Option<f32>,
}

fn main() {
    #[cfg(target_env = "msvc")]
    {
        println!("cargo:rerun-if-changed=gdclone.manifest");
        println!("cargo:rerun-if-changed=gdclone.rc");

        embed_resource::compile("gdclone.rc", embed_resource::NONE);
    }

    println!("cargo:rerun-if-changed=.git/logs/HEAD");
    println!("cargo:rerun-if-changed=assets/data/object.json");

    let version = if let Some(version) = {
        if let Ok(command_output) = Command::new("git")
            .args(["describe", "--always", "--tags", "--dirty"])
            .output()
        {
            if command_output.status.success() {
                Some(String::from_utf8(command_output.stdout).unwrap())
            } else {
                None
            }
        } else {
            None
        }
    } {
        version
    } else {
        env!("CARGO_PKG_VERSION").to_string()
    };
    println!("cargo:rustc-env=VERSION={}", version);

    let mut object_json = File::open("assets/data/object.json").unwrap();
    let mut bytes = Vec::new();
    if let Ok(metadata) = object_json.metadata() {
        bytes.reserve(metadata.len() as usize);
    }
    object_json.read_to_end(&mut bytes).unwrap();
    let hashmap: HashMap<u64, ObjectData> = serde_json::de::from_slice(&bytes).unwrap();
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let mut dest_file =
        BufWriter::new(File::create(Path::new(&out_dir).join("generated_object.rs")).unwrap());

    let mut phf_map = phf_codegen::Map::new();

    for (k, v) in hashmap {
        let mut object_string = String::new();
        write_object(v, &mut object_string);
        phf_map.entry(k, &object_string);
    }

    writeln!(
        &mut dest_file,
        "static OBJECT_DEFAULT_DATA: phf::Map<u64, ObjectDefaultData> = \n{};\n",
        phf_map.build(),
    )
    .unwrap();
}

fn write_object(object_data: ObjectData, output: &mut String) {
    output.write_str("ObjectDefaultData { ").unwrap();

    if let Some(texture) = object_data.texture {
        write_value_string("texture", &texture, output);
    } else {
        write_value_string("texture", "emptyFrame.png", output);
    }

    if let Some(default_z_layer) = object_data.default_z_layer {
        write_value("default_z_layer", default_z_layer, output);
    } else {
        write_value("default_z_layer", 0, output);
    }

    if let Some(default_z_order) = object_data.default_z_order {
        write_value("default_z_order", default_z_order, output);
    } else {
        write_value("default_z_order", 0, output);
    }

    if let Some(default_base_color_channel) = object_data.default_base_color_channel {
        write_value(
            "default_base_color_channel",
            default_base_color_channel,
            output,
        );
    } else {
        write_value_raw("default_base_color_channel", "u64::MAX", output);
    }

    if let Some(default_detail_color_channel) = object_data.default_detail_color_channel {
        write_value(
            "default_detail_color_channel",
            default_detail_color_channel,
            output,
        );
    } else {
        write_value_raw("default_detail_color_channel", "u64::MAX", output);
    }

    if let Some(color_type) = &object_data.color_type {
        write_value_raw(
            "color_kind",
            &format!("ObjectColorKind::{}", color_type),
            output,
        );
    } else {
        write_value_raw("color_kind", "ObjectColorKind::None", output);
    }

    if let Some(swap_base_detail) = object_data.swap_base_detail {
        write_value("swap_base_detail", swap_base_detail, output);
    } else {
        write_value("swap_base_detail", false, output);
    }

    if let Some(opacity) = object_data.opacity {
        write_value_f32("opacity", opacity, output);
    } else {
        write_value_f32("opacity", 1., output);
    }

    if let Some(hitbox) = &object_data.hitbox {
        write_hitbox(hitbox, output);
    } else {
        output.write_str("hitbox: None,").unwrap();
    }

    if let Some(children) = &object_data.children {
        output.write_str("children: &[").unwrap();

        if children.len() == 1 {
            write_child(&children[0], output);
        } else {
            write_child(&children[0], output);
            for child in &children[1..] {
                output.write_str(", ").unwrap();
                write_child(child, output);
            }
        }
        output.write_str("]").unwrap();
    } else {
        output.write_str("children: &[]").unwrap();
    }
    output.write_str(" }").unwrap();
}

fn write_hitbox(hitbox: &Hitbox, output: &mut String) {
    output.write_str("hitbox: ").unwrap();

    match &*hitbox.r#type {
        "Box" | "Slope" => {
            output
                .write_str(&format!("Some(HitboxData::{} {{ ", hitbox.r#type))
                .unwrap();

            if hitbox.r#type == "Box" {
                write_value_vec2("offset", hitbox.x.unwrap(), hitbox.y.unwrap(), output);
            }

            write_value_vec2(
                "half_extents",
                hitbox.width.unwrap() / 2.,
                hitbox.height.unwrap() / 2.,
                output,
            );
            output.write_str(" }), ").unwrap();
        }
        "Circle" => {
            output
                .write_str(&format!(
                    "Some(HitboxData::Circle {{ radius: {} }}), ",
                    f32_writable(hitbox.radius.unwrap())
                ))
                .unwrap();
        }
        _ => panic!(),
    }
}

fn write_child(child: &Child, output: &mut String) {
    output.write_str("ObjectChild { ").unwrap();
    write_value_string("texture", &child.texture, output);

    if child.x != 0. || child.y != 0. || child.z != 0 {
        write_value_vec3("offset", child.x, child.y, child.z as f32, output);
    } else {
        write_value_raw("offset", "Vec3::ZERO", output);
    }

    write_value_f32("rotation", child.rot, output);

    if child.anchor_x != 0. || child.anchor_y != 0. {
        write_value_vec2("anchor", child.anchor_x, child.anchor_y, output);
    } else {
        write_value_raw("anchor", "Vec2::ZERO", output);
    }

    if child.scale_x != 1. || child.scale_y != 1. {
        write_value_vec2("scale", child.scale_x, child.scale_y, output);
    } else {
        write_value_raw("scale", "Vec2::ONE", output);
    }

    write_value("flip_x", child.flip_x, output);

    write_value("flip_y", child.flip_y, output);

    if let Some(color_type) = &child.color_type {
        write_value_raw(
            "color_kind",
            &format!("ObjectColorKind::{}", color_type),
            output,
        );
    } else {
        write_value_raw("color_kind", "ObjectColorKind::None", output);
    }

    if let Some(opacity) = child.opacity {
        write_value_f32("opacity", opacity, output);
    } else {
        write_value_f32("opacity", 1., output);
    }

    if let Some(children) = &child.children {
        output.write_str("children: &[").unwrap();
        if children.len() == 1 {
            write_child(&children[0], output);
        } else {
            write_child(&children[0], output);
            for child in &children[1..] {
                output.write_str(", ").unwrap();
                write_child(child, output);
            }
        }
        output.write_str("]").unwrap();
    } else {
        output.write_str("children: &[]").unwrap();
    }
    output.write_str(" }").unwrap();
}

fn write_value_string(name: &str, value: &str, output: &mut String) {
    output
        .write_str(format!("{}: \"{}\", ", name, value).as_str())
        .unwrap();
}

fn write_value_raw(name: &str, value: &str, output: &mut String) {
    output
        .write_str(format!("{}: {}, ", name, value).as_str())
        .unwrap();
}

fn write_value_vec2(name: &str, x: f32, y: f32, output: &mut String) {
    output
        .write_str(
            format!(
                "{}: Vec2::new({}, {}), ",
                name,
                f32_writable(x),
                f32_writable(y)
            )
            .as_str(),
        )
        .unwrap();
}

fn write_value_vec3(name: &str, x: f32, y: f32, z: f32, output: &mut String) {
    output
        .write_str(
            format!(
                "{}: Vec3::new({}, {}, {}), ",
                name,
                f32_writable(x),
                f32_writable(y),
                f32_writable(z)
            )
            .as_str(),
        )
        .unwrap();
}

fn write_value_f32(name: &str, value: f32, output: &mut String) {
    output
        .write_str(format!("{}: {}, ", name, f32_writable(value)).as_str())
        .unwrap();
}

fn f32_writable(value: f32) -> String {
    if value.fract() == 0. {
        return format!("{}.", value);
    }
    format!("{}", value)
}

fn write_value<T>(name: &str, value: T, output: &mut String)
where
    T: Display,
{
    output
        .write_str(format!("{}: {}, ", name, value).as_str())
        .unwrap();
}
