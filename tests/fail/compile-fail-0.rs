///// lifetime error /////
fn main() {
    let x;
    {
        x = &String::new();
        //~  ^^^^^^^^^^^^^ error: temporary value dropped while borrowed
        //~                label: creates a temporary value which is freed while still in use
    }
    println!("{}", x);
}

////////////////////////////////////////////////////////////////////////////////
