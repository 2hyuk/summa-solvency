#![feature(generic_const_exprs)]
use halo2_proofs::halo2curves::bn256::Fr;
use halo2_solidity_verifier::SolidityGenerator;
use summa_solvency::circuits::{
    univariate_grand_sum::{NoRangeCheckConfig, UnivariateGrandSum},
    utils::generate_setup_artifacts,
};

use std::env;
use std::fs;

const K: u32 = 17;
const N_CURRENCIES: usize = 2;
const N_USERS: usize = 16;

fn main() {
    let circuit = UnivariateGrandSum::<
        N_USERS,
        N_CURRENCIES,
        NoRangeCheckConfig<N_CURRENCIES, N_USERS>,
    >::init_empty();

    let (params, pk, _) =
        generate_setup_artifacts(K, Some("../backend/ptau/hermez-raw-17"), &circuit).unwrap();

    let generator = SolidityGenerator::new(
        &params,
        pk.get_vk(),
        halo2_solidity_verifier::BatchOpenScheme::Bdfg21,
        1,
    );
    let (_, vk) = generator.render_separately().unwrap();

    // vk 값 추출
    let mut vk_values = String::new();
    for line in vk.lines() {
        if line.contains("mstore(") {
            if let Some(start) = line.find(",") {
                let hex_value = &line[start + 4..start + 68]; // 0x 다음 64자리 추출
                vk_values.push_str(hex_value);
            }
        }
    }

    // 결과 출력
    println!("\nExtracted vk values:");
    println!("{}", vk_values);
}
