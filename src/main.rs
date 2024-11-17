use clap::Parser;
use homedir::my_home;
use image::{ImageReader, Rgb, RgbImage};
use quantette::{ColorSpace, ImagePipeline, QuantizeMethod};
use rayon::prelude::*;
use std::fs::File;
use std::io::prelude::*;
use std::collections::HashSet;
use std::process::Command;

// TODO: proper error handling without .unwrap() and .panic() (use result in the main function)

#[derive(Parser, Debug)]
#[command(version, about = "Make any wallpaper fit any colorscheme", long_about = None, max_term_width=120)]
struct Args {
    /// File to generate image from
    input: String,

    /// File to generate image to
    output: String,

    /// Image Palette
    #[arg(long, short, num_args = 0..)]
    palette: Option<Vec<String>>,

    /// Use palette from pywal
    #[arg(long, short)]
    wal: bool,

    /// Use palette from Xresources
    #[arg(long, short)]
    xresources: bool,

    /// Blur the image
    #[arg(long, short)]
    blur: bool,

    /// Do not quantize the image before processing (may make the image look better)
    #[arg(long)]
    no_quantize: bool,

    /// Do not dither the image while quantizing
    #[arg(long)]
    no_dither: bool,

    /// Use average algorithm (calculate the average color of each pixel with the pixels around)
    /// to generate the wallpaper, and set the size of the box to calculate the color from.
    /// A value of 0 disables this
    #[arg(long, default_value_t = 0)]
    average: i32,
}

fn color_difference(color1: Rgb<u8>, color2: Rgb<u8>) -> u32 {
    color1
        .0 // these .0 just extract the [u8] from the Rgb datastructure
        .iter()
        .zip(color2.0.iter())
        // find the difference in all 3 colors and sum them
        .fold(0, |acc, colors: (&u8, &u8)| {
            acc + (colors.0.max(colors.1) - colors.0.min(colors.1)) as u32
        })
}

fn average_color(pixels: Vec<Rgb<u8>>) -> Rgb<u8> {
    let avg = pixels
        .iter()
        .map(|pixel| pixel.0) // at this point we have an array of rgb values
        .fold([0, 0, 0], |mut acc, pixels| {
            for x in 0..=2 {
                acc[x] += pixels[x] as usize; // now we calculate the sum for r,g,b
            }
            return acc;
        });

    let red = (avg[0] / pixels.len()).clamp(0, 255) as u8; // now we calculate the average
    let green = (avg[1] / pixels.len()).clamp(0, 255) as u8;
    let blue = (avg[2] / pixels.len()).clamp(0, 255) as u8;
    return Rgb([red, green, blue]);
}

fn decode_xresources(contents: String) -> Vec<Rgb<u8>>{
    let palette : HashSet<Rgb<u8>> = contents
        .lines()
        .map(|line| line.split(" ")) // split each line into the two colums (id and color)
        .flatten()
        .filter(|split| split.contains("#")) // only retain the color column
        .map(|substr| substr.split_inclusive("#")) // split out the hash and any text before
        .flatten()
        .filter(|split| !split.contains("#")) // only retain the hex codes
        .map(|hex_str| {
            let hex_num = u32::from_str_radix(hex_str, 16).unwrap();
            let r = (hex_num >> 16) as u8;
            let g = ((hex_num >> 8) & 0x00FF) as u8;
            let b = (hex_num & 0x0000_00FF) as u8;
            Rgb([r, g, b])
        })
        .collect();
    return palette.into_iter().collect();
}

fn xresources_load() -> Vec<Rgb<u8>>{
    use std::str;
    let xrdb_output = Command::new("xrdb")
        .arg("-query")
        .output()
        .expect("failed to execute xrdb")
        .stdout;
    let mut contents = String::new();
    contents.push_str(match str::from_utf8(&xrdb_output) {
        Ok(val) => val,
        Err(_) => panic!("got non UTF-8 data from xrdb"),
    });
    return decode_xresources(contents)
}

fn pywal_load() -> Vec<Rgb<u8>> {
    let mut xres_loc = my_home().unwrap().unwrap();
    xres_loc.push(".cache/wal/colors.Xresources");
    let mut pywal_xres = File::open(xres_loc).unwrap();
    let mut contents = String::new();
    pywal_xres.read_to_string(&mut contents).unwrap();

    return decode_xresources(contents);
 }

fn main() {
    let args = Args::parse();
    let mut input_img = ImageReader::open(args.input)
        .unwrap()
        .decode()
        .unwrap()
        .into_rgb8(); //enforce rgb8
    let mut output_img = RgbImage::new(input_img.dimensions().0, input_img.dimensions().1);
    // default palette
    let mut palette = vec![
        Rgb([0, 0, 0]),
        Rgb([29, 43, 83]),
        Rgb([126, 37, 83]),
        Rgb([0, 135, 81]),
        Rgb([171, 82, 54]),
        Rgb([95, 87, 79]),
        Rgb([194, 195, 199]),
        Rgb([255, 241, 232]),
        Rgb([255, 0, 77]),
        Rgb([255, 163, 0]),
        Rgb([255, 236, 39]),
        Rgb([0, 228, 54]),
        Rgb([41, 173, 255]),
        Rgb([131, 118, 156]),
        Rgb([255, 119, 168]),
        Rgb([255, 204, 170]),
    ];

    if args.wal {
        palette = pywal_load();
    }

    if args.xresources {
        palette = xresources_load();
    }

    if let Some(palette_input) = args.palette {
        if palette_input.len() == 0 {
            panic!("Palette input malformed")
        } else {
            let palette_hash : HashSet<Rgb<u8>> = palette_input.iter().filter(|split| split.contains("#")) // only retain the color column
                .map(|substr| substr.split_inclusive("#")) // split out the hash and any text before
                .flatten()
                .filter(|split| !split.contains("#")) // only retain the hex codes
                .map(|hex_str| {
                    let hex_num = u32::from_str_radix(hex_str, 16).unwrap();
                    let r = (hex_num >> 16) as u8;
                    let g = ((hex_num >> 8) & 0x00FF) as u8;
                    let b = (hex_num & 0x0000_00FF) as u8;
                    Rgb([r, g, b])
                })
                .collect();
            palette = palette_hash.into_iter().collect();
        }
    }

    if !args.no_quantize {
        input_img = ImagePipeline::try_from(&input_img)
            .unwrap()
            .palette_size(palette.len() as u8) // limit the no. of colors to the length of the pallet
            .dither(!args.no_dither)
            .colorspace(ColorSpace::Oklab) // use a more accurate color space
            .quantize_method(QuantizeMethod::kmeans()) // use a more accurate quantization algorithm
            .quantized_rgbimage_par(); // run the pipeline in parallel to get a [`RgbImage`]
    }
    let output: Vec<Rgb<u8>> = input_img
        .par_enumerate_pixels()
        .map(|(x, y, pixel)| {
            // lazy way of checking for averaging
            if args.average > 0 {
                // To get the average for a group of pixels, instead of using a 2d vector
                // we flatten all of
                let mut pixel_vec = Vec::<Rgb<u8>>::new();
                // get pixels within a range about the central pixel
                for row in -args.average..args.average {
                    for column in -args.average..args.average {
                        // this block is limited in image sizes and the
                        // conversions ultimately as long as nobody attempts to
                        // use a massive image we should be fine
                        if let Some(pixel) = input_img.get_pixel_checked(
                            ((x as i32) + column).clamp(0, input_img.width() as i32) as u32,
                            ((y as i32) + row).clamp(0, input_img.height() as i32) as u32,
                        ) {
                            pixel_vec.push(*pixel);
                        }
                    }
                }
                return average_color(pixel_vec);
            } else {
                return *pixel;
            }
        })
        // this map finds the closest color within the pallet and selects it
        .map(|averaged_pixel| {
            palette
                .iter()
                // this map finds the differences for all colors in the palette
                // compared to the pixel
                .map(|color| (color.clone(), color_difference(averaged_pixel, *color)))
                // this fold actually finds the closest palette color
                .fold((Rgb([0, 0, 0]), u32::MAX), |lowest_current, x| {
                    if x.1 < lowest_current.1 {
                        x
                    } else {
                        lowest_current
                    }
                })
                .0
        })
        .collect();

    // this is seperated from the main iterator because doing it within the
    // iterator would require a mutex (expensive)
    for i in 0..output.len() as u32 {
        let x = i % input_img.width();
        let y = i / input_img.width();
        output_img.put_pixel(x, y, output[i as usize])
    }
    if args.blur {
        output_img = image::imageops::blur(&output_img, 1.0);
    }
    output_img.save(args.output).unwrap();
}
