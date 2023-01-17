import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import {
  createMint,
  TOKEN_PROGRAM_ID,
  mintTo,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  Account,
} from "@solana/spl-token";
import { assert } from "chai";
import { Contracts } from "../target/types/contracts";

const DB_ID = "eriqih";

describe("contracts", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Contracts as Program<Contracts>;
  const connection = anchor.getProvider().connection;
  const userWallet = anchor.workspace.Contracts.provider.wallet;

  const initBuidl = async () => {
    const mint = await createMint(
      connection,
      userWallet.payer,
      userWallet.publicKey,
      userWallet.publicKey,
      6
    );

    const userAta = await getOrCreateAssociatedTokenAccount(
      connection,
      userWallet.payer,
      mint,
      userWallet.publicKey
    );

    await mintTo(
      connection,
      userWallet.payer,
      mint,
      userAta.address,
      userWallet.publicKey,
      1000
    );

    const buidlAccount = anchor.web3.Keypair.generate();

    const [vaultPDAAddress] = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("vault"),
        buidlAccount.publicKey.toBuffer(),
        mint.toBuffer(),
      ],
      program.programId
    );

    const [vaultAuthorityAddress] =
      anchor.web3.PublicKey.findProgramAddressSync(
        [
          Buffer.from("authority"),
          buidlAccount.publicKey.toBuffer(),
          mint.toBuffer(),
        ],
        program.programId
      );

    const tx = await program.methods
      .initializeBuidl(DB_ID)
      .accounts({
        owner: userWallet.publicKey,
        mint: mint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        buidlAccount: buidlAccount.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        vault: vaultPDAAddress,
      })
      .signers([buidlAccount])
      .rpc();

    // console.log("init buidl tx: ", tx);

    await mintTo(
      connection,
      userWallet.payer,
      mint,
      vaultPDAAddress,
      userWallet.publicKey,
      1000
    );

    let fetchedVault = await getAccount(connection, vaultPDAAddress);

    const buidlAccountData = await program.account.buidlAccount.fetch(
      buidlAccount.publicKey
    );

    assert.equal(
      buidlAccountData.vaultAccount.toString(),
      vaultPDAAddress.toString()
    );
    assert.equal(
      buidlAccountData.owner.toString(),
      userWallet.publicKey.toString()
    );
    assert.equal(buidlAccountData.token.toString(), mint.toString());
    assert.equal(buidlAccountData.dbId, DB_ID);

    assert.equal(fetchedVault.amount.toString(), "1000");
    assert.equal(
      fetchedVault.owner.toString(),
      vaultAuthorityAddress.toString()
    );

    return {
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount,
    };
  };

  const depositTokens = async (
    userAta: Account,
    vaultPDAAddress: anchor.web3.PublicKey,
    mint: anchor.web3.PublicKey,
    vaultAuthorityAddress: anchor.web3.PublicKey,
    buidlAccount: anchor.web3.Keypair
  ) => {
    const backerAccountPDA = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("backer"),
        buidlAccount.publicKey.toBuffer(),
        userWallet.publicKey.toBuffer(),
      ],
      program.programId
    )[0];

    const depositTx = await program.methods
      .deposit(new anchor.BN(500))
      .accounts({
        depositor: userWallet.publicKey,
        depositorTokenAccount: userAta.address,
        vault: vaultPDAAddress,
        mint: mint,
        tokenProgram: TOKEN_PROGRAM_ID,
        vaultAuthority: vaultAuthorityAddress,
        backerAccount: backerAccountPDA,
        buidlAccount: buidlAccount.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    // console.log("depositTx: ", depositTx);

    let fetchedVault = await getAccount(connection, vaultPDAAddress);
    let fetchedBackerAccount = await program.account.backerAccount.fetch(
      backerAccountPDA
    );

    assert.equal(fetchedVault.amount.toString(), "1500");
    assert.equal(fetchedBackerAccount.amount.toString(), "500");
    assert.equal(
      fetchedBackerAccount.address.toString(),
      userWallet.publicKey.toString()
    );
    assert.equal(
      fetchedBackerAccount.buidlAccount.toString(),
      buidlAccount.publicKey.toBase58()
    );
  };

  const createProposal = async (
    vaultPDAAddress: anchor.web3.PublicKey,
    buidlAccount: anchor.web3.Keypair
  ) => {
    const proposalAccount = anchor.web3.Keypair.generate();
    const withdrawer_token_account = anchor.web3.PublicKey.unique();

    const proposalTx = await program.methods
      .createProposal(
        new anchor.BN(1000),
        DB_ID,
        withdrawer_token_account,
        new anchor.BN(7)
      )
      .accounts({
        payer: userWallet.publicKey,
        proposalAccount: proposalAccount.publicKey,
        vault: vaultPDAAddress,
        buidlAccount: buidlAccount.publicKey,
      })
      .signers([proposalAccount])
      .rpc();

    // console.log("proposalTx: ", proposalTx);

    const proposalAccountData = await program.account.proposalAccount.fetch(
      proposalAccount.publicKey
    );

    const proposalEndDate = new Date(
      Number(proposalAccountData.endTimestamp.toString()) * 1000
    ).getDate();

    const sevenDaysFromNow = new Date(
      new Date().getTime() + 7 * 24 * 60 * 60 * 1000
    ).getDate();

    assert.equal(proposalEndDate, sevenDaysFromNow);

    assert.equal(proposalAccountData.amount.toString(), "1000");
    assert.equal(proposalAccountData.dbId, DB_ID);
    assert.equal(
      proposalAccountData.withdrawerTokenAccount.toString(),
      withdrawer_token_account.toString()
    );

    return {
      proposalAccount,
      withdrawer_token_account,
    };
  };

  const createAccountAndVote = async (
    proposal_account: anchor.web3.PublicKey,
    upvote: boolean
  ) => {
    const voterAccount = anchor.web3.Keypair.generate();

    const signature = await program.provider.connection.requestAirdrop(
      voterAccount.publicKey,
      1000000000
    );
    await program.provider.connection.confirmTransaction(signature);

    const voterPDA = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("vote"),
        proposal_account.toBuffer(),
        voterAccount.publicKey.toBuffer(),
      ],
      program.programId
    )[0];

    const voteTx = await program.methods
      .vote(upvote)
      .accounts({
        voter: voterAccount.publicKey,
        voterAccount: voterPDA,
        proposalAccount: proposal_account,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([voterAccount])
      .rpc();

    // console.log("voteTx: ", voteTx);

    return {
      voterAccount,
      voterPDA,
    };
  };

  it("Initialize Build", async () => {
    initBuidl();
  });

  it("Deposits from another wallet", async () => {
    const {
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount,
    } = await initBuidl();

    await depositTokens(
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount
    );
  });

  it("Creates a proposal", async () => {
    const {
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount,
    } = await initBuidl();

    await depositTokens(
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount
    );

    await createProposal(vaultPDAAddress, buidlAccount);
  });

  it("Fails to create a proposal if the proposal amount is higher than available amount", async () => {
    const {
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount,
    } = await initBuidl();

    await depositTokens(
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount
    );

    const proposalAccount = anchor.web3.Keypair.generate();

    const withdrawer_token_account = anchor.web3.PublicKey.unique();

    await program.methods
      .createProposal(
        new anchor.BN(3000),
        DB_ID,
        withdrawer_token_account,
        new anchor.BN(7)
      )
      .accounts({
        payer: userWallet.publicKey,
        proposalAccount: proposalAccount.publicKey,
        vault: vaultPDAAddress,
        buidlAccount: buidlAccount.publicKey,
      })
      .signers([proposalAccount])
      .rpc()
      .catch((err) => {
        assert.equal(
          err.message,
          "AnchorError occurred. Error Code: InsufficientFunds. Error Number: 6000. Error Message: Insufficient funds."
        );
      });
  });

  it("can vote on a proposal", async () => {
    const {
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount,
    } = await initBuidl();

    await depositTokens(
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount
    );

    const { proposalAccount } = await createProposal(
      vaultPDAAddress,
      buidlAccount
    );

    const voterPDA = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("vote"),
        proposalAccount.publicKey.toBuffer(),
        userWallet.publicKey.toBuffer(),
      ],
      program.programId
    )[0];

    const voteTx = await program.methods
      .vote(true)
      .accounts({
        proposalAccount: proposalAccount.publicKey,
        voter: userWallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        voterAccount: voterPDA,
      })
      .rpc();

    // console.log("voteTx: ", voteTx);

    const proposalAccountData = await program.account.proposalAccount.fetch(
      proposalAccount.publicKey
    );

    const voterAccountData = await program.account.backerVoteAccount.fetch(
      voterPDA
    );

    assert.equal(proposalAccountData.upvotes.toString(), "1");
    assert.equal(proposalAccountData.downvotes.toString(), "0");
    assert.equal(voterAccountData.upvote, true);
    assert.equal(
      voterAccountData.proposalAccount.toString(),
      proposalAccount.publicKey.toString()
    );
    assert.equal(
      voterAccountData.address.toString(),
      userWallet.publicKey.toString()
    );

    await program.methods
      .vote(true)
      .accounts({
        proposalAccount: proposalAccount.publicKey,
        voter: userWallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        voterAccount: voterPDA,
      })
      .rpc()
      .catch((err) => {
        assert.equal(
          err.message,
          "AnchorError occurred. Error Code: AlreadyVoted. Error Number: 6004. Error Message: Already voted the same vote on this proposal."
        );
      });

    const voteTx2 = await program.methods
      .vote(false)
      .accounts({
        proposalAccount: proposalAccount.publicKey,
        voter: userWallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        voterAccount: voterPDA,
      })
      .rpc();

    // console.log("voteTx2: ", voteTx2);

    const proposalAccountData2 = await program.account.proposalAccount.fetch(
      proposalAccount.publicKey
    );

    const voterAccountData2 = await program.account.backerVoteAccount.fetch(
      voterPDA
    );

    assert.equal(proposalAccountData2.upvotes.toString(), "0");
    assert.equal(proposalAccountData2.downvotes.toString(), "1");
    assert.equal(voterAccountData2.upvote, false);
    assert.equal(
      voterAccountData2.proposalAccount.toString(),
      proposalAccount.publicKey.toString()
    );

    await program.methods
      .vote(false)
      .accounts({
        proposalAccount: proposalAccount.publicKey,
        voter: userWallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        voterAccount: voterPDA,
      })
      .rpc()
      .catch((err) => {
        assert.equal(
          err.message,
          "AnchorError occurred. Error Code: AlreadyVoted. Error Number: 6004. Error Message: Already voted the same vote on this proposal."
        );
      });
  });

  it("can withdraw tokens from a proposal", async () => {
    const {
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount,
    } = await initBuidl();

    await depositTokens(
      userAta,
      vaultPDAAddress,
      mint,
      vaultAuthorityAddress,
      buidlAccount
    );

    const { proposalAccount, withdrawer_token_account } = await createProposal(
      vaultPDAAddress,
      buidlAccount
    );

    await createAccountAndVote(proposalAccount.publicKey, true);

    await createAccountAndVote(proposalAccount.publicKey, false);

    await createAccountAndVote(proposalAccount.publicKey, false);

    await createAccountAndVote(proposalAccount.publicKey, true);

    await createAccountAndVote(proposalAccount.publicKey, true);

    const checkProposalTx = await program.methods
      .checkProposal()
      .accounts({
        buidlAccount: buidlAccount.publicKey,
        proposalAccount: proposalAccount.publicKey,
        vault: vaultPDAAddress,
        tokenProgram: TOKEN_PROGRAM_ID,
        withdrawerTokenAccount: withdrawer_token_account,
        vaultAuthority: vaultAuthorityAddress,
        mint,
      })
      .rpc();
  });
});
