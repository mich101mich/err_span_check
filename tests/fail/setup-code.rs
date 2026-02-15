fn main() {
    let x = std::ptr::null_mut();

    ///// thread sharing /////
    std::thread::spawn(|| println!("{:?}", x));
    //~                ^^^^^^^^^^^^^^^^^^^^^^ `*mut _` cannot be shared between threads safely
    ////////////////////////////////////////////////////////////////////////////////

    ///// type error /////
    let y: usize = x;
    //~            ^ error: mismatched types
    //~              label: expected `usize`, found `*mut _`
    ////////////////////////////////////////////////////////////////////////////////
}
