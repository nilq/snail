mod snail;

use snail::lexer;
use snail::{Parser, Traveler};

fn main() {
    let test = r#"
fib := {
  |0| 0
  |1| 1
  |n| (fib n - 1) + fib n - 2
}
    "#;

    let lexer = lexer(&mut test.chars());

    let traveler   = Traveler::new(lexer.collect());
    let mut parser = Parser::new(traveler);
    
    match parser.parse() {
        Ok(n)  => println!("{:#?}", n),
        Err(e) => println!("{}", e),
    }
}
