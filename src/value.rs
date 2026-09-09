use std::ops::Add;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
}

impl Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a+b),
            _ => panic!("Runtime error: invalid types for Add"),

        }
    }
}


