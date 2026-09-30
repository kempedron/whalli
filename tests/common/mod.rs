use whalli::compiler::Compiler;
use whalli::lexer::Lexer;
use whalli::parser::Parser;
use whalli::vm::VM;

pub fn run_code(code: &str) -> VM {
    let mut lexer = Lexer::new(code);
    let tokens = lexer.tokenize().expect("Lexer error");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().expect("Parser error");
    let compiler = Compiler::new();
    let func_obj = compiler.compile_function(&ast);
    let mut vm = VM::new_with_function(func_obj);
    vm.run().expect("Runtime error");
    vm
}
