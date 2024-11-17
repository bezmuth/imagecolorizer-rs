use clap::Parser;
use image::{ImageReader, Rgb, RgbImage};
use quantette::{ColorSpace, ImagePipeline, QuantizeMethod};
use rayon::prelude::*;

#[derive(Parser, Debug)]
#[command(version, about = "Make any wallpaper fit any colorscheme", long_about = None, max_term_width=120)]
struct Args {
    /// File to generate image from
    input: String,

    /// File to generate image to
    output: String,

    /// Image Palette
    #[arg(long, short, num_args = 0..)]
    palette: Vec<String>,

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

fn main() {
    let args = Args::parse();

    let mut input_img = ImageReader::open(args.input)
        .unwrap()
        .decode()
        .unwrap()
        .into_rgb8(); //enforce rgb8
    let mut output_img = RgbImage::new(input_img.dimensions().0, input_img.dimensions().1);

    let palette = vec![
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
            if args.average > 0 {
                // lazy way of checking for averaging
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
        // this map finds the closest color within the pallet and applies it
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
