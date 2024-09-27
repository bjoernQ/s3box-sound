//! Play some sound on ESP32-S3-BOX

#![no_std]
#![no_main]

use embassy_executor::Spawner;

use es8311::{Config, Resolution, SampleFreq};
use esp_backtrace as _;
use esp_hal::i2s::asynch::I2sWriteDmaAsync;
use esp_hal::{
    dma::{Dma, DmaPriority},
    dma_circular_buffers,
    gpio::{Io, Level, Output},
    i2c::I2C,
    i2s::{DataFormat, I2s, Standard},
    prelude::*,
};
use esp_println::println;

const SAMPLE: &[u8] = include_bytes!("../sample.raw");

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let mut pa_ctrl = Output::new(io.pins.gpio46, Level::Low);
    pa_ctrl.set_high();

    let i2c = I2C::new(
        peripherals.I2C0,
        io.pins.gpio8,
        io.pins.gpio18,
        100u32.kHz(),
    );

    let mut es8311 = es8311::Es8311::new(i2c, es8311::Address::Primary);

    let cfg = Config {
        sample_frequency: SampleFreq::Freq44KHz,
        mclk: Some(es8311::MclkFreq::Freq2822KHz),
        res_in: Resolution::Resolution16,
        res_out: Resolution::Resolution16,
        mclk_inverted: false,
        sclk_inverted: true,
    };

    let delay = esp_hal::delay::Delay::new();
    es8311.init(delay, &cfg).unwrap();
    println!("init done");
    es8311.voice_mute(false).unwrap();
    es8311.set_voice_volume(160).unwrap();

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.channel0;

    let (tx_buffer, tx_descriptors, _, rx_descriptors) = dma_circular_buffers!(128, 0);

    let i2s = I2s::new(
        peripherals.I2S0,
        Standard::Philips,
        DataFormat::Data16Channel16,
        44100u32.Hz(),
        dma_channel.configure_for_async(false, DmaPriority::Priority0),
        tx_descriptors,
        rx_descriptors,
    );

    let i2s_tx = i2s
        .i2s_tx
        .with_bclk(io.pins.gpio17)
        .with_ws(io.pins.gpio47)
        .with_dout(io.pins.gpio15)
        .build();

    let data = SAMPLE;

    let buffer = tx_buffer;
    let mut idx = 0;
    for i in 0..usize::min(data.len(), buffer.len()) {
        buffer[i] = data[idx];
        idx = (idx + 1) % data.len();
    }

    let mut transfer = i2s_tx.write_dma_circular_async(buffer).unwrap();
    loop {
        transfer
            .push_with(|dma_buf| {
                for i in 0..dma_buf.len() {
                    dma_buf[i] = data[idx];
                    idx = (idx + 1) % data.len();
                }
                dma_buf.len()
            })
            .await
            .unwrap();
    }
}
