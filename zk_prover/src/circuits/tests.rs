#[cfg(test)]
mod test {

    use crate::merkle_sum_tree::{MerkleSumTree, Tree};
    use crate::{
        circuits::{
            merkle_sum_tree::MstInclusionCircuit,
            utils::{full_prover, full_verifier, generate_setup_artifacts},
        },
        merkle_sum_tree::{big_uint_to_fp, Entry},
    };
    use halo2_proofs::{
        dev::{FailureLocation, MockProver, VerifyFailure},
        halo2curves::bn256::Fr as Fp,
        plonk::Any,
    };
    use num_bigint::{BigUint, ToBigUint};
    use snark_verifier_sdk::CircuitExt;

    const N_ASSETS: usize = 2;
    const LEVELS: usize = 4;
    const N_BYTES: usize = 14;
    const K: u32 = 11;

    #[test]
    fn test_valid_merkle_sum_tree() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        for user_index in 0..16 {
            // get proof for entry ˆuser_indexˆ
            let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
            let user_entry = merkle_sum_tree.get_entry(user_index);

            let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
                merkle_proof,
                user_entry.clone(),
            );

            let valid_prover = MockProver::run(K, &circuit, circuit.instances()).unwrap();

            assert_eq!(circuit.instances()[0].len(), circuit.num_instance()[0]);
            assert_eq!(circuit.instances()[0].len(), 2 + N_ASSETS);

            valid_prover.assert_satisfied();
        }
    }

    #[test]
    fn test_valid_merkle_sum_tree_with_full_prover() {
        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init_empty();

        // Generate a universal trusted setup for testing purposes.
        //
        // The verification key (vk) and the proving key (pk) are then generated.
        // An empty circuit is used here to emphasize that the circuit inputs are not relevant when generating the keys.
        // Important: The dimensions of the circuit used to generate the keys must match those of the circuit used to generate the proof.
        // In this case, the dimensions are represented by the height of the Merkle tree.
        let (params, pk, vk) = generate_setup_artifacts(K, None, circuit).unwrap();

        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        // Generate the proof
        let proof = full_prover(&params, &pk, circuit.clone(), circuit.instances());

        // verify the proof to be true
        assert!(full_verifier(&params, &vk, proof, circuit.instances()));

        // the user should perform the check on the public inputs
        // public input #0 is the leaf hash
        let expected_leaf_hash = user_entry.compute_leaf().hash;
        assert_eq!(circuit.instances()[0][0], expected_leaf_hash);

        // public input #1 is the root hash
        let expected_root_hash = merkle_sum_tree.root().hash;
        assert_eq!(circuit.instances()[0][1], expected_root_hash);

        // public inputs [2, 2+N_ASSETS - 1] are the root balances
        let expected_root_balances = merkle_sum_tree.root().balances;
        for i in 0..N_ASSETS {
            assert_eq!(circuit.instances()[0][2 + i], expected_root_balances[i]);
        }
    }

    // Passing an invalid root hash in the instance column should fail the permutation check between the computed root hash and the instance column root hash
    #[test]
    fn test_invalid_root_hash() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();
        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let mut instances = circuit.instances();
        let invalid_root_hash = Fp::from(1000u64);
        instances[0][1] = invalid_root_hash;

        let invalid_prover = MockProver::run(K, &circuit, instances).unwrap();

        assert_eq!(
            invalid_prover.verify(),
            Err(vec![
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (94, "permute state").into(),
                        offset: 36
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 1 }
                },
            ])
        );
    }

    #[test]
    fn test_invalid_root_hash_as_instance_with_full_prover() {
        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init_empty();

        // generate a universal trusted setup for testing, along with the verification key (vk) and the proving key (pk).
        let (params, pk, vk) = generate_setup_artifacts(K, None, circuit).unwrap();

        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let invalid_root_hash = Fp::from(1000u64);

        let mut instances = circuit.instances();
        instances[0][1] = invalid_root_hash;

        // Generate the proof
        let proof = full_prover(&params, &pk, circuit, instances.clone());

        // verify the proof to be false
        assert!(!full_verifier(&params, &vk, proof, instances));
    }

    // Passing an invalid entry balance as input for the witness generation should fail:
    // - the permutation check between the leaf hash and the instance column leaf hash
    // - the permutation check between the computed root hash and the instance column root hash
    // - the permutations checks between the computed root balances and the instance column root balances
    #[test]
    fn test_invalid_entry_balance_as_witness() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let mut circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let instances = circuit.instances();

        let invalid_leaf_balances = [1000.to_biguint().unwrap(), 1000.to_biguint().unwrap()];

        // invalidate user entry
        let invalid_entry =
            Entry::new(circuit.entry.username().to_string(), invalid_leaf_balances).unwrap();

        circuit.entry = invalid_entry;

        let invalid_prover = MockProver::run(K, &circuit, instances).unwrap();
        assert_eq!(
            invalid_prover.verify(),
            Err(vec![
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (12, "assign nodes hashes per merkle tree level").into(),
                        offset: 0
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (94, "permute state").into(),
                        offset: 36
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (95, "assign value to perform range check").into(),
                        offset: 0
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (96, "assign value to perform range check").into(),
                        offset: 0
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 0 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 1 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 2 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 3 }
                },
            ])
        );
    }

    // Passing an invalid leaf hash in the instance column should fail the permutation check between the (valid) leaf hash added as part of the witness and the instance column leaf hash
    #[test]
    fn test_invalid_leaf_hash_as_instance() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let mut instances = circuit.instances();
        let invalid_leaf_hash = Fp::from(1000u64);
        instances[0][0] = invalid_leaf_hash;

        let invalid_prover = MockProver::run(K, &circuit, instances).unwrap();

        assert_eq!(
            invalid_prover.verify(),
            Err(vec![
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (12, "assign nodes hashes per merkle tree level").into(),
                        offset: 0
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 0 }
                },
            ])
        );
    }

    // Passing a non binary index should fail the bool constraint inside "assign nodes hashes per merkle tree level" and "assign nodes balances per asset" region and the permutation check between the computed root hash and the instance column root hash
    #[test]
    fn test_non_binary_index() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let mut circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let instances = circuit.instances();

        // invalidate path index inside the circuit
        circuit.path_indices[0] = Fp::from(2);

        let invalid_prover = MockProver::run(K, &circuit, instances).unwrap();

        assert_eq!(
            invalid_prover.verify(),
            Err(vec![
                VerifyFailure::ConstraintNotSatisfied {
                    constraint: ((6, "bool constraint").into(), 0, "").into(),
                    location: FailureLocation::InRegion {
                        region: (12, "assign nodes hashes per merkle tree level").into(),
                        offset: 0
                    },
                    cell_values: vec![(((Any::advice(), 2).into(), 0).into(), "0x2".to_string()),]
                },
                VerifyFailure::ConstraintNotSatisfied {
                    constraint: ((6, "bool constraint").into(), 0, "").into(),
                    location: FailureLocation::InRegion {
                        region: (13, "assign nodes balances per asset").into(),
                        offset: 0
                    },
                    cell_values: vec![(((Any::advice(), 2).into(), 0).into(), "0x2".to_string()),]
                },
                VerifyFailure::ConstraintNotSatisfied {
                    constraint: ((6, "bool constraint").into(), 0, "").into(),
                    location: FailureLocation::InRegion {
                        region: (16, "assign nodes balances per asset").into(),
                        offset: 0
                    },
                    cell_values: vec![(((Any::advice(), 2).into(), 0).into(), "0x2".to_string()),]
                },
                VerifyFailure::ConstraintNotSatisfied {
                    constraint: ((7, "swap constraint").into(), 0, "").into(),
                    location: FailureLocation::InRegion {
                        region: (12, "assign nodes hashes per merkle tree level").into(),
                        offset: 0
                    },
                    cell_values: vec![
                        (
                            ((Any::advice(), 0).into(), 0).into(),
                            "0xe113acd03b98f0bab0ef6f577245d5d008cbcc19ef2dab3608aa4f37f72a407"
                                .to_string()
                        ),
                        (
                            ((Any::advice(), 0).into(), 1).into(),
                            "0x17ef9d8ee0e2c8470814651413b71009a607a020214f749687384a7b7a7eb67a"
                                .to_string()
                        ),
                        (
                            ((Any::advice(), 1).into(), 0).into(),
                            "0x17ef9d8ee0e2c8470814651413b71009a607a020214f749687384a7b7a7eb67a"
                                .to_string()
                        ),
                        (
                            ((Any::advice(), 1).into(), 1).into(),
                            "0xe113acd03b98f0bab0ef6f577245d5d008cbcc19ef2dab3608aa4f37f72a407"
                                .to_string()
                        ),
                        (((Any::advice(), 2).into(), 0).into(), "0x2".to_string()),
                    ]
                },
                VerifyFailure::ConstraintNotSatisfied {
                    constraint: ((7, "swap constraint").into(), 0, "").into(),
                    location: FailureLocation::InRegion {
                        region: (13, "assign nodes balances per asset").into(),
                        offset: 0
                    },
                    cell_values: vec![
                        (((Any::advice(), 0).into(), 0).into(), "0x2e70".to_string()),
                        (((Any::advice(), 0).into(), 1).into(), "0x108ef".to_string()),
                        (((Any::advice(), 1).into(), 0).into(), "0x108ef".to_string()),
                        (((Any::advice(), 1).into(), 1).into(), "0x2e70".to_string()),
                        (((Any::advice(), 2).into(), 0).into(), "0x2".to_string()),
                    ]
                },
                VerifyFailure::ConstraintNotSatisfied {
                    constraint: ((7, "swap constraint").into(), 0, "").into(),
                    location: FailureLocation::InRegion {
                        region: (16, "assign nodes balances per asset").into(),
                        offset: 0
                    },
                    cell_values: vec![
                        (((Any::advice(), 0).into(), 0).into(), "0xa0cb".to_string()),
                        (((Any::advice(), 0).into(), 1).into(), "0x48db".to_string()),
                        (((Any::advice(), 1).into(), 0).into(), "0x48db".to_string()),
                        (((Any::advice(), 1).into(), 1).into(), "0xa0cb".to_string()),
                        (((Any::advice(), 2).into(), 0).into(), "0x2".to_string()),
                    ]
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (94, "permute state").into(),
                        offset: 36
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 1 }
                },
            ])
        );
    }

    // Swapping the indices should fail the permutation check between the computed root hash and the instance column root hash
    #[test]
    fn test_swapping_index() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let mut circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let instances = circuit.instances();

        // swap indices
        circuit.path_indices[0] = Fp::from(1);

        let invalid_prover = MockProver::run(K, &circuit, instances).unwrap();

        assert_eq!(
            invalid_prover.verify(),
            Err(vec![
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (94, "permute state").into(),
                        offset: 36
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 1 }
                },
            ])
        );
    }

    // Adding a balance at the verge of overflowing should fail the range check for any following computed sum and, because we are adding a fake balance.
    // Furthermore, the public input check on the root hash and on root_balances[0] should fail too
    #[test]
    fn test_balance_not_in_range() {
        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        // Only now we can instantiate the circuit with the actual inputs
        let mut circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let balance = BigUint::from(2u64).pow(N_BYTES as u32 * 8) - BigUint::from(1u64);

        circuit.path_element_balances[0][0] = big_uint_to_fp(&balance); // 2^64 - 1. It means that as soon as it is summed with the other balances, it will overflow

        let invalid_prover = MockProver::run(K, &circuit, circuit.instances()).unwrap();

        assert_eq!(
            invalid_prover.verify(),
            Err(vec![
                VerifyFailure::Permutation {
                    column: (Any::Fixed, 2).into(),
                    location: FailureLocation::OutsideRegion { row: 431 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Fixed, 2).into(),
                    location: FailureLocation::OutsideRegion { row: 739 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Fixed, 2).into(),
                    location: FailureLocation::OutsideRegion { row: 1047 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Fixed, 2).into(),
                    location: FailureLocation::OutsideRegion { row: 1355 }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (35, "assign value to perform range check").into(),
                        offset: 14
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (56, "assign value to perform range check").into(),
                        offset: 14
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (77, "assign value to perform range check").into(),
                        offset: 14
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (94, "permute state").into(),
                        offset: 36
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (95, "assign value to perform range check").into(),
                        offset: 0
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::advice(), 0).into(),
                    location: FailureLocation::InRegion {
                        region: (95, "assign value to perform range check").into(),
                        offset: 14
                    }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 1 }
                },
                VerifyFailure::Permutation {
                    column: (Any::Instance, 0).into(),
                    location: FailureLocation::OutsideRegion { row: 2 }
                },
            ])
        );
    }

    #[cfg(feature = "dev-graph")]
    #[test]
    fn print_mst_inclusion() {
        use plotters::prelude::*;

        let merkle_sum_tree =
            MerkleSumTree::<N_ASSETS, N_BYTES>::new("src/merkle_sum_tree/csv/entry_16.csv")
                .unwrap();

        let user_index = 0;

        let merkle_proof = merkle_sum_tree.generate_proof(user_index).unwrap();
        let user_entry = merkle_sum_tree.get_entry(user_index);

        let circuit = MstInclusionCircuit::<LEVELS, N_ASSETS, N_BYTES>::init(
            merkle_proof,
            user_entry.clone(),
        );

        let root = BitMapBackend::new("prints/mst-inclusion-layout.png", (2048, 32768))
            .into_drawing_area();
        root.fill(&WHITE).unwrap();
        let root = root
            .titled("Merkle Sum Tree Inclusion Layout", ("sans-serif", 60))
            .unwrap();

        halo2_proofs::dev::CircuitLayout::default()
            .render(K, &circuit, &root)
            .unwrap();
    }
}
