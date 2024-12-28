#![no_std]
#![no_main]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_semihosting;
extern crate stm32l4xx_hal as hal;

use crate::hal::{
    gpio::{gpioc::PC13, Edge, ExtiPin, Input, PullUp},
    interrupt,
    prelude::*,
    stm32,
};
use core::cell::{Cell, RefCell};
use core::ops::DerefMut;
use cortex_m::{
    interrupt::{free, Mutex},
    peripheral::NVIC,
};
use cortex_m_semihosting::hprintln;
use rt::{
    entry,
    exception,
    ExceptionFrame,
};

enum SystemState{
    Sleep,
    Standalone,
    Continuous,
}

// Set up global state. It's all mutexed up for concurrency safety.
static BUTTON: Mutex<RefCell<Option<PC13<Input<PullUp>>>>> = Mutex::new(RefCell::new(None));
static PRESS_COUNT: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));
static _SYSTEM_STATE: Mutex<Cell<SystemState>> = Mutex::new(Cell::new(SystemState::Standalone));

#[entry]
fn main() -> ! {
    if let Some(mut dp) = stm32::Peripherals::take() {
        dp.RCC.apb2enr.write(|w| w.syscfgen().set_bit());

        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain(); // .constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);

        rcc.cfgr
            .hclk(48.MHz())
            .sysclk(80.MHz())
            .pclk1(24.MHz())
            .pclk2(24.MHz())
            .freeze(&mut flash.acr, &mut pwr);

        // Create a button input with an interrupt
        hprintln!("Maybe fail");
        let mut gpioc = dp.GPIOC.split(&mut rcc.ahb2);
        let mut board_btn = gpioc
            .pc13
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);
        board_btn.make_interrupt_source(&mut dp.SYSCFG, &mut rcc.apb2);
        board_btn.enable_interrupt(&mut dp.EXTI);
        board_btn.trigger_on_edge(&mut dp.EXTI, Edge::Falling);

        hprintln!("Made it");

        // Move button to global context
        free(|cs| {
            BUTTON.borrow(cs).replace(Some(board_btn));
        });
        hprintln!("Buttons moved to global context");

        // Enable interrupts
        unsafe {
            NVIC::unmask(stm32::Interrupt::EXTI15_10);
        }     

        hprintln!("Interrupts enables");

        let mut press_count = free(|cs| {return PRESS_COUNT.borrow(cs).get();});
        loop {
            let new_count = free(|cs| {return PRESS_COUNT.borrow(cs).get();});
            if new_count != press_count {
                //indicate it
                hprintln!("Press Count: {}", new_count);
                press_count = new_count;
            }
        }
    }

    loop {
        continue;
    }
}

#[interrupt]
fn EXTI15_10() {
    free(|cs| {
        let mut btn_ref = BUTTON.borrow(cs).borrow_mut();
        if let Some(ref mut btn) = btn_ref.deref_mut() {
            if btn.check_interrupt() {
                btn.clear_interrupt_pending_bit();
            }
        }
        let count = PRESS_COUNT.borrow(cs);
        let inc = count.get() + 1;
        count.replace(inc);
    });
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("Hard Fault happens: {:#?}", ef);
}
