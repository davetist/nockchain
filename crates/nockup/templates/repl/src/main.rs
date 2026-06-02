use std::error::Error;
use std::fs;
use std::io::{self, Write};

use bytes::Bytes;
use nockapp::kernel::boot;
use nockapp::noun::slab::NounSlab;
use nockapp::wire::{SystemWire, Wire};
use nockapp::{AtomExt, NockApp};
use nockvm::noun::{Atom, D, T};
use nockvm_macros::tas;

fn string_to_atom(slab: &mut NounSlab, s: &str) -> Result<Atom, Box<dyn Error>> {
    let bytes = Bytes::from(s.as_bytes().to_vec());
    Ok(Atom::from_bytes(slab, &bytes))
}

async fn process_input(nockapp: &mut NockApp, input: &str) -> Result<String, Box<dyn Error>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(String::new());
    }
    if matches!(input, "exit" | "x" | "q" | "quit") {
        return Err("Exit command received".into());
    }

    let mut poke_slab = NounSlab::new();
    let str_atom = string_to_atom(&mut poke_slab, input)?;
    let command_noun = T(&mut poke_slab, &[D(tas!(b"call")), str_atom.as_noun()]);
    poke_slab.set_root(command_noun);

    match nockapp.poke(SystemWire.to_wire(), poke_slab).await {
        Ok(effects) => Ok(format!("{} effect(s)", effects.len())),
        Err(e) => Ok(format!("command failed: {}", e)),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = boot::default_boot_cli(false);
    boot::init_default_tracing(&cli);

    let kernel = fs::read("out.jam").map_err(|e| format!("Failed to read out.jam: {}", e))?;

    let mut nockapp: NockApp = boot::setup(&kernel, cli, &[], "{{project_name}}", None).await?;

    loop {
        print!("repl> ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => match process_input(&mut nockapp, input.trim()).await {
                Ok(result) => println!("{}", result),
                Err(e) => {
                    if e.to_string().contains("Exit command received") {
                        println!("Exiting...");
                        break;
                    }
                    println!("Error: {}", e);
                }
            },
            Err(_) => {
                println!("Closing program...");
                break;
            }
        }
    }

    Ok(())
}
