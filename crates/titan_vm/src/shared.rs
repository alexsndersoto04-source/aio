//! Copy-on-write storage for heap values (arrays, tuples, maps, struct fields).
//!
//! Titan tiene semántica de VALOR: `let b = a` y luego modificar `b` no cambia
//! `a`. Antes la VM la implementaba copiando en profundidad el contenedor en
//! cada lectura de una variable (`PushLocal` clonaba el `Vec` completo), lo que
//! hacía O(n) leer `xs[i]` y O(n²) recorrer un array. `Shared<T>` conserva la
//! misma semántica observable, pero comparte el contenido con un `Arc` y solo
//! copia cuando alguien modifica un valor que otra referencia todavía ve
//! (`Arc::make_mut`). Leer y pasar valores pasa a ser O(1).

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

#[derive(Default)]
pub struct Shared<T>(Arc<T>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Shared(Arc::new(value))
    }

    /// Número de referencias vivas (útil en pruebas de copy-on-write).
    pub fn ref_count(this: &Self) -> usize {
        Arc::strong_count(&this.0)
    }

    /// `true` si ambos comparten exactamente el mismo almacenamiento.
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}

impl<T: Clone> Shared<T> {
    /// Extrae el contenido. Sin copia si esta es la única referencia.
    pub fn into_inner(self) -> T {
        Arc::try_unwrap(self.0).unwrap_or_else(|shared| (*shared).clone())
    }

    /// Acceso mutable; copia el contenido solo si está compartido.
    pub fn make_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(Arc::clone(&self.0))
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}

impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}

impl<T: fmt::Debug> fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (*self.0).fmt(f)
    }
}

impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Shared::new(value)
    }
}

impl<A, T: FromIterator<A>> FromIterator<A> for Shared<T> {
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> Self {
        Shared::new(iter.into_iter().collect())
    }
}

impl<T: Clone + IntoIterator> IntoIterator for Shared<T> {
    type Item = T::Item;
    type IntoIter = T::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.into_inner().into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Shared<T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;
    type IntoIter = <&'a T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (&*self.0).into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::Shared;

    #[test]
    fn clone_shares_and_write_copies() {
        let a: Shared<Vec<i32>> = Shared::new(vec![1, 2, 3]);
        let mut b = a.clone();
        assert!(Shared::ptr_eq(&a, &b));
        b.push(4);
        assert!(!Shared::ptr_eq(&a, &b));
        assert_eq!(*a, vec![1, 2, 3]);
        assert_eq!(*b, vec![1, 2, 3, 4]);
    }

    #[test]
    fn unique_owner_mutates_in_place() {
        let mut a: Shared<Vec<i32>> = Shared::new(Vec::with_capacity(8));
        a.push(1);
        let before = a.as_ptr();
        a.push(2);
        assert_eq!(before, a.as_ptr());
        assert_eq!(Shared::ref_count(&a), 1);
        assert_eq!(a.into_inner(), vec![1, 2]);
    }
}
