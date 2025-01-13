#![no_std]
#![no_main]

// Example for an STM32 using Embassy.

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::Pull;
use embassy_stm32::spi::{self, Spi, Config};
use embassy_stm32::mode::Async;
use embassy_stm32::time::Hertz;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // 1. Initialize the board-peripherals with Embassy
    let p = embassy_stm32::init(Default::default());

    // 2. Configure an input pin for NSS with pull-up (typical for active-low).
    //    Then wrap it in an ExtiInput for interrupts on edges.
    let mut nss_irq = ExtiInput::new(p.PA4, p.EXTI4, Pull::None);
    // 3. Create SPI config in slave mode.
    let mut cfg = Config::default();
    cfg.is_slave = true;                 // <--- slave mode
    cfg.mode.polarity = spi::Polarity::IdleHigh;   // e.g. mode 3, adjust to match your master
    cfg.mode.phase = spi::Phase::CaptureOnSecondTransition;
    // For a slave, frequency is usually ignored, but set something anyway:
    cfg.frequency = Hertz(1_000_000);
    // If your board needs internal pull on MISO, set cfg.miso_pull = Pull::Up/Down

    // 4. Create an async SPI peripheral in slave mode with DMA.
    //    Adjust pins (SPI1, PB5, etc.) to match your hardware.
    let mut spi = Spi::new(
        p.SPI1,
        p.PA5,               // SCK
        p.PA7,               // MOSI
        p.PA6,               // MISO
        p.DMA1_CH3,          // TX DMA channel
        p.DMA1_CH2,          // RX DMA channel
        cfg,
    );

    // 5. Spawn a background task that waits for NSS to go low, then reads data via SPI DMA
    spawner.spawn(spi_slave_task(nss_irq, spi)).unwrap();
}

// This async task is triggered by NSS transitions. Once the master pulls NSS low,
// we read from SPI using DMA, then log the data.
#[embassy_executor::task]
async fn spi_slave_task(mut nss_irq: ExtiInput<'static>, mut spi: Spi<'static, Async>) {
    // We’ll read up to 32 bytes from the master. Adjust as needed!
    let mut rx_buf = [0u8; 32];

    loop {
        info!("Waiting for NSS to go LOW (master select)...");
        // 1. Wait for NSS falling edge. (Master is selecting us)
        nss_irq.wait_for_falling_edge().await;

        // 2. Perform a DMA-based read. The master must now clock out 32 bytes (or however many).
        info!("NSS low detected; starting SPI read...");
        match spi.read(&mut rx_buf).await {
            Ok(()) => {
                info!("SPI read completed. Received: {:x}", rx_buf);
            }
            Err(e) => {
                warn!("SPI read error: {:?}", e);
            }
        }

        // 3. (Optional) Wait for NSS rising edge if you want to confirm the transaction ended.
        //    Something like:
        // nss_irq.wait_for_rising_edge().await;
        // info!("NSS returned high (master done).");
    }
}