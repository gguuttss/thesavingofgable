Protocol consists of 4 phases.

**First phase: initialization**
Joost (or the one with Gables admin/owner badge) initializes the protocol while supplying it with Gables admin/owner badge. He then receives the saving protocol's admin badge, to use the protocols protected methods.


**Second phase: proof of supply gathering**
Victims of Gable's error, who still have locked LSUs in the protocol, can participate in the upcoming saving by supplying the component with their Proof of Supply NFT Receipt handed out by Gable.
They can do this through the insert_proof() method. The saving protocol then creates an NFT Receipt, to prove that you've inserted your Proof of Supply.
If for some reason they decide they want to remove their Proof of Supply again, this is possible through the withdraw_proof() method, in combination with the NFT Receipt.


**Third phase: saving, started by calling start_saving()**
Through the use of the admin badge it is possible to call the claim_xrd() method, to get xrd into the liquidity_pool_vault again, and make sure total_pool > owner_liquidity.
This allows for more lsu_withdraws. In the same transaction, the method save_next() should be called as often as possible, ensuring total_pool < owner_liquidity again, non-component-owned Proof of Supplies can not call withdraw_lsu() independently.
The cycle of claim_xrd() and save_next() continues until we are satisfied with the amount of LSUs saved.


**Fourth phase: collection, started by calling finish_saving()**
An x amount of LSUs and XRD is now collected by the protocol. All participants can now collect their share of the saved LSUs and XRD (which is calculated to be (single_user_locked_lsu_amount / all_users_locked_lsu_amount)% of the collected funds).
Collecting can be done by calling retrieve_reward() together with their NFT Receipts.
