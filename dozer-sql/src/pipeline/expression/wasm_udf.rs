use crate::pipeline::errors::PipelineError;
use crate::pipeline::errors::PipelineError::UnsupportedSqlError;
use crate::pipeline::errors::UnsupportedSqlError::GenericError;
use crate::pipeline::expression::execution::Expression;

use dozer_types::types::{Field, FieldType, Record, Schema};
use dozer_types::ordered_float::OrderedFloat;


use std::env;

const MODULE_NAME: &str = "wasm_udf";


use wasmtime::*;


pub fn evaluate_wasm_udf(
    schema: &Schema,
    name: &str,
    args: &[Expression],
    return_type: &FieldType,
    record: &Record,
) -> Result<Field, PipelineError> {
    let values = args
        .iter()
        .take(args.len() - 1)
        .map(|arg| arg.evaluate(record, schema))
        .collect::<Result<Vec<_>, PipelineError>>()?;

    let values2: Vec<Val> = values
        .iter()
        .map(|field| match field {
            Field::Int(value) =>  Val::I64(*value),
            Field::Float(value) =>  Val::F64(value.to_bits()),
            _ => todo!(),
        })
        .collect();

    let engine = Engine::default();

    let env_path = env::var("DOZER_WASM_UDF").map_err(|_| {
        PipelineError::InvalidFunction("Missing 'DOZER_WASM_UDF' environment var".to_string())
    })?;

    let module = Module::from_file(&engine, env_path)?;
    let mut store = Store::new(&engine, 4);
    let instance = Instance::new(&mut store, &module, &[])?;

    let wasm_udf_func = instance.get_func(&mut store, name).expect("export wasn't a function");
    let mut results: [Val; 1] = [Val::I64(0)];

    // match wasm_udf_func.call(&mut store, &[Val::I64(9)], &mut results) {
    match wasm_udf_func.call(&mut store, &values2, &mut results) {
        Ok(()) => {}
        Err(trap) => {
            panic!("execution of wasm_udf_func `{}` resulted in a wasm trap: {}", name, trap);
        }
    }

    Ok(match return_type {
        FieldType::Int => Field::Int(results[0].i64().expect("Oops")),
        FieldType::Float => Field::Float(OrderedFloat(results[0].f64().expect("Oops"))),
        FieldType::UInt
        | FieldType::Binary
        | FieldType::U128
        | FieldType::I128
        | FieldType::Boolean
        | FieldType::String
        | FieldType::Text
        | FieldType::Decimal
        | FieldType::Date
        | FieldType::Timestamp
        | FieldType::Point
        | FieldType::Duration
        | FieldType::Json => {
            return Err(UnsupportedSqlError(GenericError(
                "Unsupported return type for wasm udf".to_string(),
            )))
        }
    })
}
