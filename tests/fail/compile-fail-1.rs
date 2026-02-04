///// type error /////
fn main() {
    let x: usize;
    {
        x = &String::new();
        //~ ^^^^^^^^^^^^^^ error: mismatched types
        //~                label: expected `usize`, found `&String`
    }
    println!("{}", x);
}

////////////////////////////////////////////////////////////////////////////////
