use super::SsaError;
pub(super) fn reserve<T>(v: &mut Vec<T>, n: usize) -> Result<(), SsaError> {
    let end = v.len().checked_add(n).ok_or(SsaError::PlatformCapacity)?;
    let size = size_of::<T>();
    if size != 0 && end > isize::MAX as usize / size {
        return Err(SsaError::PlatformCapacity);
    }
    v.try_reserve(n).map_err(SsaError::Allocation)
}
pub(super) fn push<T>(v: &mut Vec<T>, x: T) -> Result<(), SsaError> {
    reserve(v, 1)?;
    v.push(x);
    Ok(())
}
pub(super) fn filled<T: Copy>(n: usize, x: T) -> Result<Vec<T>, SsaError> {
    let mut v = Vec::new();
    reserve(&mut v, n)?;
    v.resize(n, x);
    Ok(v)
}
pub(super) fn lists<T>(n: usize) -> Result<Vec<Vec<T>>, SsaError> {
    let mut v = Vec::new();
    reserve(&mut v, n)?;
    for _ in 0..n {
        v.push(Vec::new());
    }
    Ok(v)
}
pub(super) fn id(n: usize) -> Result<u32, SsaError> {
    u32::try_from(n).map_err(|_| SsaError::PlatformCapacity)
}
pub(super) fn bug(message: &'static str) -> SsaError {
    SsaError::InternalInvariant(message)
}
