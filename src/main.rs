use clap::*;
use ihex::*;
use ihex_ext::*;
use simple_logger::SimpleLogger;
use std::fs;

mod part;
pub use part::*;

mod programmer;
pub use programmer::*;

mod util;
pub use util::*;

fn cli() -> Command {
    return Command::new("sinowealth-kb-tool")
        .about("A programming tool for Sinowealth Gaming KB devices")
        .version("0.0.1")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .author("Karolis Stasaitis")
        .subcommand(
            Command::new("read")
                .short_flag('r')
                .about("Read flash contents.")
                .arg(arg!(output_file: <OUTPUT_FILE> "file to write flash contents to"))
                .arg(
                    arg!(-p --part <PART>)
                        .value_parser(PARTS.keys().map(|&s| s).collect::<Vec<_>>())
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("write")
                .short_flag('w')
                .about("Write file into flash.")
                .arg(arg!(input_file: <INPUT_FILE> "payload to write into flash"))
                .arg(
                    arg!(-p --part <PART>)
                        .value_parser(PARTS.keys().map(|&s| s).collect::<Vec<_>>())
                        .required(true),
                ),
        );
}

fn main() {
    SimpleLogger::new().init().unwrap();

    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("read", sub_matches)) => {
            let part_name = sub_matches
                .get_one::<String>("part")
                .map(|s| s.as_str())
                .unwrap();

            let output_file = sub_matches
                .get_one::<String>("output_file")
                .map(|s| s.as_str())
                .unwrap();

            let part = PARTS.get(part_name).unwrap();

            let result = Programmer::new(part).read_cycle();

            let ihex = result.to_ihex();

            let obj = create_object_file_representation(&ihex).unwrap();
            fs::write(output_file, obj).expect("Unable to write file");
        }
        Some(("write", sub_matches)) => {
            let input_file = sub_matches
                .get_one::<String>("input_file")
                .map(|s| s.as_str())
                .unwrap();

            let part_name = sub_matches
                .get_one::<String>("part")
                .map(|s| s.as_str())
                .unwrap();

            let part = PARTS.get(part_name).unwrap();

            let (mut firmware, _) = load_file_vec(input_file, part.flash_size, 0).unwrap();

            Programmer::new(part).write_cycle(&mut firmware);
        }
        Some(("erase", sub_matches)) => {
            let part_name = sub_matches
                .get_one::<String>("part")
                .map(|s| s.as_str())
                .unwrap();

            let part = PARTS.get(part_name).unwrap();

            Programmer::new(part).erase_cycle();
        }
        _ => unreachable!(),
    }
}
