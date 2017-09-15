mod snail;

use snail::lexer;
use snail::{Parser, Traveler};

fn main() {
    let test = r#"
t
    "#;
    
    let lexer = lexer(&mut test.chars());

    let traveler   = Traveler::new(lexer.collect());
    let mut parser = Parser::new(traveler);
    
    match parser.parse() {
        Ok(n)  => println!("{:#?}", n),
        Err(e) => println!("{}", e),
    }
}
