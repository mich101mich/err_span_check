fn main() {
    let x;
    {
        x = &String::new();
    }
    println!("{}", x);
}
