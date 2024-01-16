use scrypto::prelude::*;

#[derive(ScryptoSbor, NonFungibleData)]
struct ProofData {
    lsus_supplied: Decimal,
    corresponding_id: NonFungibleLocalId,
    kvs_placement: i64,
    used: bool,
}

#[derive(ScryptoSbor, NonFungibleData)]
struct LiquiditySupplier {
    pub box_nr: u64,
    pub lsu_amount: Decimal,
}

#[blueprint]
mod gabling {
    enable_method_auth! {
        methods {
            insert_proof => PUBLIC;
            withdraw_proof => PUBLIC;
            start_saving => restrict_to: [OWNER];
            claim_xrd => restrict_to: [OWNER];
            save_next => restrict_to: [OWNER];
            finish_saving => restrict_to: [OWNER];
            retrieve_reward => PUBLIC;
        }
    }

    extern_blueprint! {
        "package_tdx_2_1p50j7463yhtpmq8e9t4vklw8jfuccl0xhe7g2564w8w74nrmrsacxs",
        Flashloanpool {
            fn withdraw_lsu(&mut self, pool_nft: Bucket) -> (Bucket, Bucket);
            fn claim_xrd(&mut self, validator: Global<Validator>) -> ();
        }
    }

    const GABLE: Global<Flashloanpool> = global_component!(Flashloanpool, "component_tdx_2_1cpekt6s65g8025zgstwx4t0tpdsegafse0vtjnfms9k07mcmnr96cm");

    struct Gabling {
        // Define what resources and data will be managed by Hello components
        supply_proofs: Vault, //vault which will hold all nft proofs of supply
        supply_proofs_amount: Decimal, //a counter for the amount of total LSUs supplied by the nft proofs of supply
        supply_proofs_counter: u64, //amount of people who have supplied their nft proof to use as NonFungibleLocalId for receipts
        saving_counter: i64,
        kvs_entries: i64,
        proofs_kvs: KeyValueStore<i64, NonFungibleLocalId>,
        gable_lsus: Vault, //a vault that holds the recovered Gable lsus
        gable_owner_badge_vault: Vault, //the gable owner badge, which will be used to call claim_xrd() method of Gable component
        proof_receipt_manager: ResourceManager, //resource manager for nfts used as receipts for people putting in their proofs of supply
        gable: Global<Flashloanpool>, //the gable component
        process_stage: i64,
        earnings_vault: Vault,
        final_earnings: Decimal,
        final_lsus: Decimal,
    }

    impl Gabling {
        pub fn instantiate(proof_address: ResourceAddress, gable_lsu_address: ResourceAddress, owner_badge: Bucket) -> (Global<Gabling>, Bucket) {

            let (address_reservation, component_address) = Runtime::allocate_component_address(Gabling::blueprint_id());

            let proof_receipt_manager: ResourceManager = ResourceBuilder::new_integer_non_fungible::<ProofData>(OwnerRole::None)
            .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                non_fungible_data_updater => rule!(require(global_caller(component_address)));
                non_fungible_data_updater_updater => rule!(deny_all);
            ))
            .mint_roles(mint_roles!( 
                minter => rule!(require(global_caller(component_address)));
                minter_updater => rule!(deny_all);
            ))
            .burn_roles(burn_roles!( 
                burner => rule!(require(global_caller(component_address)));
                burner_updater => rule!(deny_all);
            ))
            .create_with_no_initial_supply();

            let admin_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata! (
                    init {
                        "name" => "admin badge", locked;
                        "symbol" => "admin", locked;
                    }
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(deny_all);
                    minter_updater => rule!(deny_all);
                ))
                .mint_initial_supply(1).into();

            let component = Self {
                supply_proofs: Vault::new(proof_address),
                supply_proofs_amount: dec!(0),
                supply_proofs_counter: 0,
                saving_counter: 0,
                kvs_entries: 0,
                proofs_kvs: KeyValueStore::new(),
                gable_lsus: Vault::new(gable_lsu_address),
                gable_owner_badge_vault: Vault::with_bucket(owner_badge),
                proof_receipt_manager,
                gable: GABLE,
                process_stage: 0,
                earnings_vault: Vault::new(XRD),
                final_earnings: dec!(0),
                final_lsus: dec!(0),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::None)
            .with_address(address_reservation)
            .globalize();

            (component, admin_badge)
        }

        pub fn insert_proof(&mut self, supply_proof: Bucket) -> Bucket {
            assert!(
                self.process_stage == 0,
                "Can not insert anymore. Process has started already."
            );
            assert_eq!(
                self.supply_proofs.resource_address(),
                supply_proof.resource_address(),
                "Non valid supply proof nft."
            );
            
            let supply_proof_manager: ResourceManager = supply_proof.resource_manager();
            let supply_id: NonFungibleLocalId = supply_proof.as_non_fungible().non_fungible_local_id();
            let proof_data: LiquiditySupplier = supply_proof_manager.get_non_fungible_data(&supply_id);
            let lsus_amount: Decimal = proof_data.lsu_amount;
            self.supply_proofs_amount += lsus_amount.clone();

            let receipt = ProofData {
                lsus_supplied: lsus_amount.clone(),
                corresponding_id: supply_id.clone(),
                kvs_placement: self.kvs_entries,
                used: false,
            };

            let receipt: Bucket = self.proof_receipt_manager.mint_non_fungible(
                &NonFungibleLocalId::integer(self.supply_proofs_counter),
                receipt
            );

            self.proofs_kvs.insert(self.kvs_entries, supply_id.clone());

            self.supply_proofs_counter += 1;
            self.supply_proofs.put(supply_proof);
            self.saving_counter += 1;
            self.kvs_entries += 1;

            receipt
        }

        pub fn withdraw_proof(&mut self, supply_proof_receipt: NonFungibleBucket) -> Bucket {
            assert!(
                self.process_stage == 0,
                "Can not withdraw anymore. The saving process is underway."
            );

            assert_eq!(
                supply_proof_receipt.resource_address(),
                self.proof_receipt_manager.address(),
                "This is a non-valid receipt."
            );

            let local_id: NonFungibleLocalId = supply_proof_receipt.non_fungible_local_id();
            let data: ProofData = supply_proof_receipt.resource_manager().get_non_fungible_data(&local_id);
            self.supply_proofs_amount -= data.lsus_supplied;
            let return_bucket: Bucket = self.supply_proofs.as_non_fungible().take_non_fungible(&data.corresponding_id).into();

            self.proofs_kvs.remove(&data.kvs_placement);
            let to_move_id: NonFungibleLocalId = self.proofs_kvs.get(&(self.kvs_entries-1)).unwrap().clone();
            //TYPING FROM MOBILE: Forgot to update NFT data, "kvs_placement" here. If not added this would lead to problems in withdrawal of moved id.
            self.proofs_kvs.remove(&(self.kvs_entries-1));
            self.proofs_kvs.insert(data.kvs_placement.clone(), to_move_id);

            self.kvs_entries -= 1;

            supply_proof_receipt.burn();
            return_bucket
        }

        pub fn start_saving(&mut self) -> () {
            self.process_stage = 1;
        }

        pub fn claim_xrd(&mut self, validator: Global<Validator>) {
            self.gable_owner_badge_vault.as_fungible().authorize_with_amount(dec!(1), || self.gable.claim_xrd(validator));
        }

        pub fn save_next(&mut self) -> () {
            assert!(
                self.process_stage == 1,
                "Can not save. Process stage is not right."
            );
            let to_save_local_id: NonFungibleLocalId = self.proofs_kvs.get(&self.saving_counter).unwrap().clone();
            let proof_bucket: Bucket = self.supply_proofs.as_non_fungible().take_non_fungible(&to_save_local_id).into();
            let (earnings_bucket, lsus_bucket): (Bucket, Bucket) = self.gable.withdraw_lsu(proof_bucket);
            self.saving_counter += 1;
            self.earnings_vault.put(earnings_bucket);
            self.gable_lsus.put(lsus_bucket);
        }

        pub fn finish_saving(&mut self) -> () {
            self.process_stage = 2;
            self.final_earnings = self.earnings_vault.amount();
            self.final_lsus = self.gable_lsus.amount();
        }

        pub fn retrieve_reward(&mut self, supply_proof_receipt: Bucket) -> (Bucket, Bucket) {
            assert!(
                self.process_stage == 2,
                "Saving process not yet finished. Try later."
            );

            assert_eq!(
                supply_proof_receipt.resource_address(),
                self.proof_receipt_manager.address(),
                "This is a non-valid receipt."
            );

            let local_id: NonFungibleLocalId = supply_proof_receipt.as_non_fungible().non_fungible_local_id();
            let data: ProofData = supply_proof_receipt.resource_manager().get_non_fungible_data(&local_id);
            supply_proof_receipt.resource_manager().update_non_fungible_data(&local_id, "used", true);
            let fraction: Decimal = data.lsus_supplied / self.supply_proofs_amount;  
            supply_proof_receipt.burn();

            (self.gable_lsus.take(fraction * self.final_lsus),
            self.earnings_vault.take(fraction * self.final_earnings))
        }
    }
}
