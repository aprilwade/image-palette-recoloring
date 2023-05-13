use image::io::Reader as ImageReader;
use image::Rgb;
use std::fmt::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use image_palette_recoloring::{compute_palette, DecomposedImage, ImageWeights};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    GeneratePalette {
        #[arg(short, long, default_value_t = 2.0 / 255.0)]
        error_bound: f64,
        #[arg(
            short,
            long,
            default_value_t = 4,
            value_parser = clap::builder::RangedU64ValueParser::<u8>::new().range(4..),
        )]
        min_size: u8,
        #[arg(value_name = "INPUT_IMAGE")]
        input_image: PathBuf,
    },
    RecolorImage {
        #[arg(short, long, value_parser = clap::builder::ValueParser::new(parse_color_list))]
        decomposition_palette: ColorList,
        #[arg(short, long)]
        input_image: PathBuf,
        #[arg(short, long, value_parser = clap::builder::ValueParser::new(parse_color_list))]
        reconstruction_palette: ColorList,
        #[arg(short, long)]
        output_image: PathBuf,
        #[arg(short = 'c', long, default_value_t = false)]
        save_individual_channels: bool,
    }
}

#[derive(Debug, Clone)]
struct ColorList(Vec<Rgb<u8>>);

impl std::ops::Deref for ColorList {
    type Target = [Rgb<u8>];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn parse_color_list(list: &str) -> Result<ColorList, String> {
    let mut colors = vec![];
    for (i, color_str) in list.split(',').enumerate() {
        if color_str.len() != 6 {
            Err(format!("Color {i} isn't valid: wrong length."))?
        }
        if !color_str.is_ascii() {
            Err(format!("Color list contains non-ascii characters."))?
        }
        colors.push(Rgb([
            u8::from_str_radix(&color_str[..2], 16)
                .map_err(|e| format!("Invalid R comp. in color {i}: {e}"))?,
            u8::from_str_radix(&color_str[2..4], 16)
                .map_err(|e| format!("Invalid G comp. in color {i}: {e}"))?,
            u8::from_str_radix(&color_str[4..], 16)
                .map_err(|e| format!("Invalid B comp. in color {i}: {e}"))?,
        ]));
    }
    Ok(ColorList(colors))
}

fn main_inner() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.commands {
        Commands::GeneratePalette { error_bound, min_size, input_image } => {
            let img = ImageReader::open(&input_image)
                .unwrap().decode().unwrap();
            let img = img.into_rgb8();
            let palette = compute_palette(&img, min_size as usize, error_bound);
            // let palette = image_recoloring::compute_palette_with_size(&img, 4);
            let palette_hex: Vec<String> = palette.iter()
                .map(|color| format!("{:02x}{:02x}{:02x}", color.0[0], color.0[1], color.0[2]))
                .collect();
            println!("{}", palette_hex.join(","));
        },
        Commands::RecolorImage {
            decomposition_palette,
            input_image,
            reconstruction_palette,
            output_image,
            save_individual_channels,
        } => {
            if decomposition_palette.len() != reconstruction_palette.len() {
                panic!("The decomposition_palette and reconstruction_palette must be the same size.")
            }
            let img = ImageReader::open(&input_image).unwrap().decode().unwrap();
            let img = img.into_rgb8();
            let weights = ImageWeights::new(&img);
            let decomposed = DecomposedImage::new(&weights, &decomposition_palette).unwrap();

            let reconstructed_img = decomposed.reconstruct(&reconstruction_palette).unwrap();
            reconstructed_img.save(&output_image).unwrap();

            if save_individual_channels {
                let filename_stem = output_image.file_stem().unwrap();
                let filename_extension = output_image.extension().unwrap();
                let mut dir = output_image.clone();
                dir.pop();
                for (i, color) in decomposition_palette.iter().enumerate() {
                    let channel_img = decomposed.get_channel_grayscale(i).unwrap();
                    let [r, g, b] = &color.0;

                    let mut filename = filename_stem.to_os_string();
                    write!(filename, "_channel_{i}_{:02X}{:02X}{:02X}.", r, g, b).unwrap();
                    filename.push(filename_extension);
                    channel_img.save(dir.join(filename)).unwrap();
                }
            }
        }
    }
    Ok(())
}

fn main() {
    match main_inner() {
        Ok(()) => (),
        Err(e) => println!("Fuck: {e}"),
    }
}
