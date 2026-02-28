use pyo3::prelude::*;

#[pymodule]
fn _ipextract(_py: Python<'_>, _m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
