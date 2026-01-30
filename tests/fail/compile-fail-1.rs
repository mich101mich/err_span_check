fn main() {
    let x: usize;
    {
        x = &String::new();
    }
    println!("{}", x);
}
