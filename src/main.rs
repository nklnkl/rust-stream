use ffmpeg_next as ffmpeg;
use dotenv::dotenv;
use std::env;

fn main() -> Result<(), ffmpeg::Error> {
    dotenv().ok();

    ffmpeg::init()?;

    // Define the input for screen capture based on your operating system.
    #[cfg(target_os = "linux")]
    let input_device = ":0.0"; // X11 display on Linux

    #[cfg(target_os = "windows")]
    let input_device = "desktop"; // Desktop capture on Windows

    #[cfg(target_os = "linux")]
    let input_format = "x11grab"; // FFmpeg screen capture input for Linux

    #[cfg(target_os = "windows")]
    let input_format = "gdigrab"; // FFmpeg screen capture input for Windows

    // Set up the input context (screen capture instead of file input)
    let mut ictx = ffmpeg::format::input_with(&input_device, input_format)?;

    // Replace the output path with the RTMP URL
    let stream_key = env::var("STREAM_KEY").unwrap();
    let rtmp_url = format!("rtmp://jfk.contribute.live-video.net/app/{}", stream_key);

    // Initialize the output context using the `flv` format for RTMP
    let mut octx = ffmpeg::format::output_as(&rtmp_url, "flv")?;

    let input_video_stream = ictx.streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let decoder = ffmpeg::codec::context::Context::from_parameters(input_video_stream.parameters())?
        .decoder()
        .video()?;

    // Change the codec to H.264 for streaming (widely used for RTMP streams)
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)?;
    let global = octx.format().flags().contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER);

    let mut ost = octx.add_stream(Some(codec))?;
    let mut encoder = codec.encoder().video()?;

    encoder.set_height(decoder.height());
    encoder.set_width(decoder.width());
    encoder.set_time_base((1, 30)); // Adjust based on your needs
    encoder.set_format(ffmpeg::format::Pixel::YUV420P);

    // RTMP-specific encoding settings (adjust as needed)
    encoder.set_bit_rate(5_000_000); // 5 Mbps for streaming quality
    encoder.set_max_b_frames(3);

    if global {
        encoder.set_flags(ffmpeg::codec::flag::Flags::GLOBAL_HEADER);
    }

    let mut encoder = encoder.open_as(codec)?;
    ost.set_parameters(encoder.parameters());

    octx.write_header()?; // Write RTMP stream header

    // Process packets from input and send them to the RTMP server
    for (stream, mut packet) in ictx.packets() {
        if stream.index() == input_video_stream.index() {
            let mut decoded = ffmpeg::frame::Video::empty();
            if decoder.decode(&packet, &mut decoded).is_ok() {
                let mut encoded = ffmpeg::Packet::empty();
                if encoder.encode(&decoded, &mut encoded).is_ok() {
                    encoded.set_stream(0);
                    encoded.rescale_ts(input_video_stream.time_base(), ost.time_base());
                    encoded.write_interleaved(&mut octx)?; // Send the encoded packet to RTMP server
                }
            }
        }
    }

    octx.write_trailer()?; // Finalize the RTMP stream

    Ok(())
}
