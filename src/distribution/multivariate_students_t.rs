use crate::distribution::Continuous;
use crate::distribution::{Normal, ChiSquared};
use crate::distribution::MultivariateNormal;
use crate::function::gamma;
use crate::statistics::{Max, MeanN, Min, Mode, VarianceN};
use crate::{Result, StatsError};
use nalgebra::Cholesky;
use nalgebra::{DMatrix, DVector};
use rand::Rng;
use std::f64::consts::{E, PI};

/// Implements the [Multivariate Student's t-distribution](https://en.wikipedia.org/wiki/Multivariate_t-distribution)
/// distribution using the "nalgebra" crate for matrix operations
///
/// Assumes all the marginal distributions have the same degree of freedom, ν
///
/// # Examples
///
/// ```
/// use statrs::distribution::{MultivariateStudent, Continuous};
/// use nalgebra::{DVector, DMatrix};
/// use statrs::statistics::{MeanN, VarianceN};
///
/// let mvs = MultivariateStudent::new(vec![0., 0.], vec![1., 0., 0., 1.], 4.).unwrap();
/// assert_eq!(mvs.mean().unwrap(), DVector::from_vec(vec![0., 0.]));
/// assert_eq!(mvs.variance().unwrap(), DMatrix::from_vec(2, 2, 4. * vec![1., 0., 0., 1.]));
/// assert_eq!(mvs.pdf(&DVector::from_vec(vec![1.,  1.])), 0.047157020175376416);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MultivariateStudent {
    dim: usize,
    scale_chol_decomp: DMatrix<f64>,
    location: DVector<f64>,
    scale: DMatrix<f64>,
    freedom: f64,
    precision: DMatrix<f64>,
    pdf_const: f64,
}

impl MultivariateStudent {
    pub fn new(location: Vec<f64>, scale: Vec<f64>, freedom: f64) -> Result<Self> {
        let dim = location.len();
        let location =  DVector::from_vec(location);
        let scale = DMatrix::from_vec(dim, dim, scale);

        // Check that the provided scale matrix is symmetric
        if scale.lower_triangle() != scale.upper_triangle().transpose()
        // Check that mean and covariance do not contain NaN
            || location.iter().any(|f| f.is_nan())
            || scale.iter().any(|f| f.is_nan())
        // Check that the dimensions match
            || location.nrows() != scale.nrows() || scale.nrows() != scale.ncols()
        // Check that the degrees of freedom is not NaN
            || freedom.is_nan()
        {
            return Err(StatsError::BadParams);
        }
        // Check that degrees of freedom is positive
        if freedom <= 0. {
            return Err(StatsError::ArgMustBePositive("Degrees of freedom must be positive"))
        }

        let scale_det = scale.determinant();
        let pdf_const = gamma::gamma((freedom + (dim as f64)) / 2.) *
            (gamma::gamma(freedom / 2.) * 
             (freedom.powi(dim as i32) * PI.powi(dim as i32) * scale_det.abs())
            .sqrt())
            .recip();

        match Cholesky::new(scale.clone()) {
            None => Err(StatsError::BadParams),
            Some(cholesky_decomp) => {
                let precision = cholesky_decomp.inverse();
                Ok(MultivariateStudent {
                    dim,
                    scale_chol_decomp: cholesky_decomp.unpack(),
                    location,
                    scale,
                    freedom,
                    precision,
                    pdf_const,
                })
            }
        }
    }
}

impl ::rand::distributions::Distribution<DVector<f64>> for MultivariateStudent {
    /// Samples from the multivariate student distribution
    ///
    /// # Formula
    ///
    /// W * L * Z + μ
    ///
    /// where `W` has √(ν/Sν) distribution, Sν has Chi-squared 
    /// distribution with ν degrees of freedom,
    /// `L` is the Cholesky decomposition of the scale matrix,
    /// `Z` is a vector of normally distributed random variables, and
    /// `μ` is the location vector
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> DVector<f64> {
        let d = Normal::new(0., 1.).unwrap();
        let s = ChiSquared::new(self.freedom).unwrap();
        let w = (self.freedom / s.sample(rng)).sqrt();
        let z = DVector::<f64>::from_distribution(self.dim, &d, rng);
        (w * &self.scale_chol_decomp * z) + &self.location
    }
}

impl Min<DVector<f64>> for MultivariateStudent {
    /// Returns the minimum value in the domain of the
    /// multivariate student's t distribution represented by a real vector
    fn min(&self) -> DVector<f64> {
        DVector::from_vec(vec![f64::NEG_INFINITY; self.dim])
    }
}

impl Max<DVector<f64>> for MultivariateStudent {
    /// Returns the maximum value in the domain of the
    /// multivariate student distribution represented by a real vector
    fn max(&self) -> DVector<f64> {
        DVector::from_vec(vec![f64::INFINITY; self.dim])
    }
}

impl MeanN<DVector<f64>> for MultivariateStudent {
    /// Returns the mean of the student distribution
    ///
    /// # Remarks
    ///
    /// This is the same mean used to construct the distribution if 
    /// the degrees of freedom is larger than 1.
    fn mean(&self) -> Option<DVector<f64>> {
        if self.freedom > 1. {
            let mut vec = vec![];
            for elt in self.location.clone().into_iter() {
                vec.push(*elt);
            }
            Some(DVector::from_vec(vec))
        } else {
            None
        }
    }
}

impl VarianceN<DMatrix<f64>> for MultivariateStudent {
    /// Returns the covariance matrix of the multivariate student distribution
    fn variance(&self) -> Option<DMatrix<f64>> {
        if self.freedom > 2. {
            Some(self.scale.clone() * self.freedom / (self.freedom - 2.))
        } else {
            None
        }
    }
}

impl Mode<DVector<f64>> for MultivariateStudent {
    /// Returns the mode of the multivariate student distribution
    ///
    /// # Formula
    ///
    /// ```ignore
    /// μ
    /// ```
    ///
    /// where `μ` is the location
    fn mode(&self) -> DVector<f64> {
        self.location.clone()
    }
}

impl<'a> Continuous<&'a DVector<f64>, f64> for MultivariateStudent {
    /// Calculates the probability density function for the multivariate
    /// student distribution at `x`
    ///
    /// # Formula
    ///
    /// ```ignore
    /// Gamma[(ν+p)/2] / [Gamma(ν/2) ((ν * π)^p det(Σ))^(1 / 2)] * [1 + 1/ν transpose(x - μ)  inv(Σ) (x - μ)]^(-(ν+p)/2)
    /// ```
    ///
    /// where `ν` is the degrees of freedom,  `μ` is the mean, `Gamma`
    /// is the Gamma function, `inv(Σ)` 
    /// is the precision matrix, `det(Σ)` is the determinant
    /// of the scale matrix, and `k` is the dimension of the distribution
    ///
    /// TODO: Make this converge for large degrees of freedom
    /// Current commented code beneath fails since `MultivariateNormal::new` accepts Vec<f64> and
    /// not DVector or DMatrix. Should implement that instead of changing back to Vec<f64>, or 
    /// even have a constructor `MultivariateNormal::from_student`.
    fn pdf(&self, x: &'a DVector<f64>) -> f64 {
        // if self.freedom == f64::INFINITY {
        //     let mvn = MultivariateNormal::new(self.location, self.scale).unwrap();
        //     return mvn.pdf(x);
        // }
        let dv = x - &self.location;
        let base_term = 1. + 1. / self.freedom
            * *(&dv.transpose() * &self.precision * &dv)
                .get((0, 0))
                .unwrap();
        self.pdf_const * base_term.powf(-(self.freedom + self.dim as f64) / 2.)
    }

    /// Calculates the log probability density function for the multivariate
    /// student distribution at `x`. Equivalent to pdf(x).ln().
    fn ln_pdf(&self, x: &'a DVector<f64>) -> f64 {
        let dv = x - &self.location;
        let base_term = 1. + 1. / self.freedom
            * *(&dv.transpose() * &self.precision * &dv)
                .get((0, 0))
                .unwrap();
        self.pdf_const.ln() - (self.freedom + self.dim as f64) / 2. * base_term.ln()
    }
}

#[rustfmt::skip]
#[cfg(all(test, feature = "nightly"))]
mod tests  {
    use crate::distribution::MultivariateNormal;
    use core::fmt::Debug;

    fn try_create(location: Vec<f64>, scale: Vec<f64>, freedom: f64) -> MultivariateStudent
    {
        let mvs = MultivariateStudent::new(location, scale, freedom);
        assert!(mvs.is_ok());
        mvs.unwrap()
    }

    fn create_case(location: Vec<f64>, scale: Vec<f64>, freedom: f64)
    {
        let mvs = try_create(location.clone(), scale.clone(), freedom);
        assert_eq!(DVector::from_vec(location.clone()), mvs.location);
        assert_eq!(DMatrix::from_vec(location.len(), location.len(), scale), mvs.scale);
    }

    fn bad_create_case(location: Vec<f64>, scale: Vec<f64>, freedom: f64)
    {
        let mvs = MultivariateStudent::new(location, scale, freedom);
        assert!(mvs.is_err());
    }

    fn test_case<T, F>(location: Vec<f64>, scale: Vec<f64>, freedom: f64, expected: T, eval: F)
    where
        T: Debug + PartialEq,
        F: FnOnce(MultivariateStudent) -> T,
    {
        let mvs = try_create(location, scale, freedom);
        let x = eval(mvs);
        assert_eq!(expected, x);
    }

    fn test_almost<F>(
        location: Vec<f64>,
        scale: Vec<f64>,
        freedom: f64,
        expected: f64,
        acc: f64,
        eval: F,
        ) where
        F: FnOnce(MultivariateStudent) -> f64,
    {
        let mvs = try_create(location, scale, freedom);
        let x = eval(mvs);
        assert_almost_eq!(expected, x, acc);
    }

    fn test_almost_multivariate_normal<F1, F2>(
        location: Vec<f64>,
        scale: Vec<f64>,
        acc: f64,
        x: DVector<f64>,
        eval_mvs: F1,
        eval_mvn: F2,
        ) where
            F1: FnOnce(MultivariateStudent, DVector<f64>) -> f64,
            F2: FnOnce(MultivariateNormal, DVector<f64>) -> f64,
        {
        let mvs = try_create(location.clone(), scale.clone(), f64::INFINITY);
        let mvn0 = MultivariateNormal::new(location, scale);
        assert!(mvn0.is_ok());
        let mvn = mvn0.unwrap();
        let mvs_x = eval_mvs(mvs, x.clone());
        let mvn_x = eval_mvn(mvn, x.clone());
        assert_almost_eq!(mvs_x, mvn_x, acc);
    }

    use super::*;

    macro_rules! dvec {
        ($($x:expr),*) => (DVector::from_vec(vec![$($x),*]));
    }

    macro_rules! mat2 {
        ($x11:expr, $x12:expr, $x21:expr, $x22:expr) => (DMatrix::from_vec(2,2,vec![$x11, $x12, $x21, $x22]));
    }

    // macro_rules! mat3 {
    //     ($x11:expr, $x12:expr, $x13:expr, $x21:expr, $x22:expr, $x23:expr, $x31:expr, $x32:expr, $x33:expr) => (DMatrix::from_vec(3,3,vec![$x11, $x12, $x13, $x21, $x22, $x23, $x31, $x32, $x33]));
    // }

    #[test]
    fn test_create() {
        create_case(vec![0., 0.], vec![1., 0., 0., 1.], 1.);
        create_case(vec![10.,  5.], vec![2., 1., 1., 2.], 3.);
        create_case(vec![4., 5., 6.], vec![2., 1., 0., 1., 2., 1., 0., 1., 2.], 14.);
        create_case(vec![0., f64::INFINITY], vec![1., 0., 0., 1.], f64::INFINITY);
        create_case(vec![0., 0.], vec![f64::INFINITY, 0., 0., f64::INFINITY], 0.1);
    }

    #[test]
    fn test_bad_create() {
        // scale not symmetric
        bad_create_case(vec![0., 0.], vec![1., 1., 0., 1.], 1.);
        // scale not positive-definite
        bad_create_case(vec![0., 0.], vec![1., 2., 2., 1.], 1.);
        // NaN in location
        bad_create_case(vec![0., f64::NAN], vec![1., 0., 0., 1.], 1.);
        // NaN in scale Matrix
        bad_create_case(vec![0., 0.], vec![1., 0., 0., f64::NAN], 1.);
        // NaN in freedom
        bad_create_case(vec![0., 0.], vec![1., 0., 0., 1.], f64::NAN);
        // Non-positive freedom
        bad_create_case(vec![0., 0.], vec![1., 0., 0., 1.], 0.);
    }

    #[test]
    fn test_variance() {
        let variance = |x: MultivariateStudent| x.variance().unwrap();
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 3., 3. * mat2![1., 0., 0., 1.], variance);
        test_case(vec![0., 0.], vec![f64::INFINITY, 0., 0., f64::INFINITY], 3., mat2![f64::INFINITY, 0., 0., f64::INFINITY], variance);
    }

    #[test]
    fn test_bad_variance() {
        let variance = |x: MultivariateStudent| x.variance();
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 2., None, variance);
    }

    #[test]
    fn test_mode() {
        let mode = |x: MultivariateStudent| x.mode();
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 1., dvec![0.,  0.], mode);
        test_case(vec![f64::INFINITY, f64::INFINITY], vec![1., 0., 0., 1.], 1., dvec![f64::INFINITY,  f64::INFINITY], mode);
    }

    #[test]
    fn test_mean() {
        let mean = |x: MultivariateStudent| x.mean().unwrap();
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 2., dvec![0., 0.], mean);
    }

    #[test]
    fn test_bad_mean() {
        let mean = |x: MultivariateStudent| x.mean();
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 1., None, mean);
    }

    #[test]
    fn test_min_max() {
        let min = |x: MultivariateStudent| x.min();
        let max = |x: MultivariateStudent| x.max();
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 1., dvec![f64::NEG_INFINITY, f64::NEG_INFINITY], min);
        test_case(vec![0., 0.], vec![1., 0., 0., 1.], 1., dvec![f64::INFINITY, f64::INFINITY], max);
        test_case(vec![10., 1.], vec![1., 0., 0., 1.], 1., dvec![f64::NEG_INFINITY, f64::NEG_INFINITY], min);
        test_case(vec![-3., 5.], vec![1., 0., 0., 1.], 1., dvec![f64::INFINITY, f64::INFINITY], max);
    }

    #[test]
    fn test_pdf() {
        let pdf = |arg: DVector<f64>| move |x: MultivariateStudent| x.pdf(&arg);
        test_almost(vec![0., 0.], vec![1., 0., 0., 1.], 4., 0.047157020175376416, 1e-15, pdf(dvec![1., 1.]));
        test_almost(vec![0., 0.], vec![1., 0., 0., 1.], 2., 0.012992240252399619, 1e-17, pdf(dvec![1., 2.]));
        test_almost(vec![2., 1.], vec![5., 0., 0., 1.], 2.5, 2.639780816598878e-5, 1e-19, pdf(dvec![1., 10.]));
        test_almost(vec![-1., 0.], vec![2., 1., 1., 6.], 1.5, 6.438051574348526e-5, 1e-19, pdf(dvec![10., 10.]));
        test_case(vec![-1., 0.], vec![f64::INFINITY, 0., 0., f64::INFINITY], 10., 0., pdf(dvec![10., 10.]));
    }

    #[test]
    fn test_ln_pdf() {
        let pdf = |arg: DVector<f64>| move |x: MultivariateStudent| x.ln_pdf(&arg);
        test_almost(vec![0., 0.], vec![1., 0., 0., 1.], 4., -3.0542723907338383, 1e-14, pdf(dvec![1., 1.]));
        test_almost(vec![0., 0.], vec![1., 0., 0., 1.], 2., -4.3434030034000815, 1e-14, pdf(dvec![1., 2.]));
        test_almost(vec![2., 1.], vec![5., 0., 0., 1.], 2.5, -10.542229575274265, 1e-14, pdf(dvec![1., 10.]));
        test_almost(vec![-1., 0.], vec![2., 1., 1., 6.], 1.5, -9.650699521198622, 1e-14, pdf(dvec![10., 10.]));
        test_case(vec![-1., 0.], vec![f64::INFINITY, 0., 0., f64::INFINITY], 10., f64::NEG_INFINITY, pdf(dvec![10., 10.]));
    }

    // TODO: These tests fail because inf degrees of freedom give NaN
    #[test]
    fn test_pdf_freedom_inf() {
        let pdf_mvs = |mv: MultivariateStudent, arg: DVector<f64>| mv.pdf(&arg);
        let pdf_mvn = |mv: MultivariateNormal, arg: DVector<f64>| mv.pdf(&arg);
        test_almost_multivariate_normal(vec![0., 0.,], vec![1., 0., 0., 1.], 1e-14, dvec![1., 1.], pdf_mvs, pdf_mvn)
    }
}
