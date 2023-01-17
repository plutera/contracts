use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Mint, SetAuthority, Token, TokenAccount, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const DISCRIMINATOR_LENGTH: usize = 8;
const PUBLIC_KEY_LENGTH: usize = 32;
const STRING_LENGTH_PREFIX: usize = 4;
const STRING_CHAR_MULTIPLIER: usize = 4;
const DB_ID_LENGTH: usize = STRING_LENGTH_PREFIX + (24 * STRING_CHAR_MULTIPLIER);
const U64_LENGTH: usize = 8;
const I64_LENGTH: usize = 8;
const BOOL_LENGTH: usize = 1;

#[program]
pub mod contracts {

    use anchor_spl::token::transfer;

    use super::*;

    pub fn initialize_buidl(ctx: Context<InitializeBuidl>, db_id: String) -> Result<()> {
        let buidl_account: &mut Account<BuidlAccount> = &mut ctx.accounts.buidl_account;
        let owner: &Signer = &ctx.accounts.owner;
        let mint = &ctx.accounts.mint;
        let vault = &ctx.accounts.vault;

        msg!("enter ix");

        let (vault_authority, _vault_authority_bump) = Pubkey::find_program_address(
            &[
                b"authority",
                buidl_account.key().as_ref(),
                mint.key().as_ref(),
            ],
            ctx.program_id,
        );

        msg!("vault_authority: {:?}", vault_authority);

        buidl_account.owner = *owner.key;
        buidl_account.db_id = db_id;
        buidl_account.vault_account = vault.key();
        buidl_account.token = mint.key();

        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(vault_authority.key()),
        )?;

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let depositor = &ctx.accounts.depositor;
        let vault = &ctx.accounts.vault;
        let depoitor_token_account = &ctx.accounts.depositor_token_account;
        let token_program = &ctx.accounts.token_program;
        let backer_account = &mut ctx.accounts.backer_account;
        let buidl_account = &ctx.accounts.buidl_account;

        let clock = Clock::get()?;

        if amount.lt(&1) {
            return Err(ErrorCode::AmountTooLow.into());
        }

        if backer_account.address.eq(&Pubkey::default()) {
            backer_account.address = depositor.key();
        }

        if backer_account.buidl_account.eq(&Pubkey::default()) {
            backer_account.buidl_account = buidl_account.key();
        }

        if backer_account.since_timestamp.eq(&0) {
            backer_account.since_timestamp = clock.unix_timestamp;
        }

        match backer_account.amount.checked_add(amount) {
            Some(new_amount) => backer_account.amount = new_amount,
            None => return Err(ErrorCode::Overflow.into()),
        }

        transfer(
            CpiContext::new(
                token_program.to_account_info(),
                Transfer {
                    from: depoitor_token_account.to_account_info(),
                    to: vault.to_account_info(),
                    authority: depositor.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }

    pub fn create_proposal(
        ctx: Context<CreateProposal>,
        amount: u64,
        db_id: String,
        withdrawer_token_account: Pubkey,
        end_after_days: i64,
    ) -> Result<()> {
        let buidl_account = &ctx.accounts.buidl_account;
        let proposal_account = &mut ctx.accounts.proposal_account;
        let vault = &ctx.accounts.vault;

        let clock = Clock::get()?;

        if amount > vault.amount {
            return Err(ErrorCode::InsufficientFunds.into());
        }

        if end_after_days < 3 {
            return Err(ErrorCode::ProposalTooShort.into());
        }

        proposal_account.buidl_account = buidl_account.key();
        proposal_account.db_id = db_id;
        proposal_account.amount = amount;
        proposal_account.upvotes = 0;
        proposal_account.downvotes = 0;

        proposal_account.end_timestamp = clock.unix_timestamp + (end_after_days * 86400);
        proposal_account.withdrawer_token_account = withdrawer_token_account;

        Ok(())
    }

    pub fn vote(ctx: Context<Vote>, upvote: bool) -> Result<()> {
        let proposal_account = &mut ctx.accounts.proposal_account;
        let voter_account = &mut ctx.accounts.voter_account;

        let clock = Clock::get()?;

        if !voter_account.voted {
            voter_account.proposal_account = proposal_account.key();
            voter_account.timestamp = clock.unix_timestamp;
            voter_account.address = ctx.accounts.voter.key();

            if upvote {
                voter_account.upvote = true;
                proposal_account.upvotes += 1;
            } else {
                voter_account.upvote = false;
                proposal_account.downvotes += 1;
            }
        } else {
            voter_account.timestamp = clock.unix_timestamp;

            if upvote {
                if voter_account.upvote {
                    return Err(ErrorCode::AlreadyVoted.into());
                } else {
                    proposal_account.downvotes -= 1;
                }

                voter_account.upvote = true;
                proposal_account.upvotes += 1;
            } else {
                if !voter_account.upvote {
                    return Err(ErrorCode::AlreadyVoted.into());
                } else {
                    proposal_account.upvotes -= 1;
                }

                voter_account.upvote = false;
                proposal_account.downvotes += 1;
            }
        }

        voter_account.voted = true;

        Ok(())
    }

    pub fn check_proposal(ctx: Context<CheckProposal>) -> Result<()> {
        let proposal_account = &ctx.accounts.proposal_account;
        let vault = &ctx.accounts.vault;
        let token_program = &ctx.accounts.token_program;
        let buidl_account = &ctx.accounts.buidl_account;
        let vault_authority = &ctx.accounts.vault_authority;
        let mint = &ctx.accounts.mint;

        let clock = Clock::get()?;

        // if clock.unix_timestamp > proposal_account.end_timestamp {
        if proposal_account.upvotes <= proposal_account.downvotes {
            return Err(ErrorCode::ProposalNotPassed.into());
        }

        let withdrawer_token_account = &ctx.accounts.withdrawer_token_account;

        transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: vault.to_account_info(),
                    to: withdrawer_token_account.to_account_info(),
                    authority: vault_authority.to_account_info(),
                },
                &[&[
                    b"authority",
                    buidl_account.key().as_ref(),
                    mint.key().as_ref(),
                ]],
            ),
            proposal_account.amount,
        )?;
        // } else {
        //     return Err(ErrorCode::ProposalNotOver.into());
        // }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeBuidl<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(init, payer = owner, space = BuidlAccount::LEN)]
    pub buidl_account: Account<'info, BuidlAccount>,
    #[account()]
    pub system_program: Program<'info, System>,
    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"vault".as_ref(), buidl_account.key().as_ref(), mint.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = owner
    )]
    pub vault: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    #[account(
        mut,
        seeds = [b"authority", buidl_account.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    /// CHECK: we are not writing to this account
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: we are not writing to this account
    #[account(mut)]
    pub mint: AccountInfo<'info>,
    #[account(mut)]
    pub depositor: Signer<'info>,
    /// CHECK: we are not writing to this account
    #[account(mut)]
    pub depositor_token_account: AccountInfo<'info>,
    pub buidl_account: Account<'info, BuidlAccount>,
    #[account(
        init_if_needed,
        payer = depositor,
        space = BackerAccount::LEN,
        seeds = [b"backer", buidl_account.key().as_ref(), depositor.key().as_ref()],
        bump
    )]
    pub backer_account: Account<'info, BackerAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub buidl_account: Account<'info, BuidlAccount>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    #[account(init, payer = payer, space = ProposalAccount::LEN)]
    pub proposal_account: Account<'info, ProposalAccount>,
    pub vault: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct Vote<'info> {
    #[account(mut)]
    pub proposal_account: Account<'info, ProposalAccount>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(
        init_if_needed,
        payer = voter,
        space = BackerAccount::LEN,
        seeds = [b"vote", proposal_account.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub voter_account: Account<'info, BackerVoteAccount>,
}

#[derive(Accounts)]
pub struct CheckProposal<'info> {
    #[account(mut)]
    pub proposal_account: Account<'info, ProposalAccount>,
    #[account(mut)]
    pub buidl_account: Account<'info, BuidlAccount>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    /// CHECK: we are just depositting tokens
    pub withdrawer_token_account: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"authority", buidl_account.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    /// CHECK: we are not writing to this account
    pub vault_authority: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
}

#[account]
pub struct BuidlAccount {
    pub owner: Pubkey,
    pub db_id: String,
    pub vault_account: Pubkey,
    pub token: Pubkey,
}

impl BuidlAccount {
    pub const LEN: usize = DISCRIMINATOR_LENGTH // discriminator
        + PUBLIC_KEY_LENGTH // owner
        + DB_ID_LENGTH // DB ID
        + PUBLIC_KEY_LENGTH // vault account
        + PUBLIC_KEY_LENGTH; // token
}

#[account]
pub struct ProposalAccount {
    pub buidl_account: Pubkey,
    pub db_id: String,
    pub amount: u64,
    pub upvotes: u64,
    pub downvotes: u64,
    pub withdrawer_token_account: Pubkey,
    pub end_timestamp: i64,
}

impl ProposalAccount {
    pub const LEN: usize = DISCRIMINATOR_LENGTH // discriminator
        + PUBLIC_KEY_LENGTH // buidl account
        + DB_ID_LENGTH // DB ID
        + U64_LENGTH // amount
        + U64_LENGTH // upvotes
        + U64_LENGTH // downvotes
        + PUBLIC_KEY_LENGTH // send to
        + I64_LENGTH; // end timestamp
}

#[account]
pub struct BackerAccount {
    pub address: Pubkey,
    pub amount: u64,
    pub since_timestamp: i64,
    pub buidl_account: Pubkey,
}

impl BackerAccount {
    pub const LEN: usize = DISCRIMINATOR_LENGTH // discriminator
        + PUBLIC_KEY_LENGTH // address
        + U64_LENGTH // amount
        + I64_LENGTH // since timestamp
        + PUBLIC_KEY_LENGTH; // buidl account
}

#[account]
pub struct BackerVoteAccount {
    pub address: Pubkey,
    pub proposal_account: Pubkey,
    pub upvote: bool,
    pub timestamp: i64,
    pub voted: bool,
}

impl BackerVoteAccount {
    pub const LEN: usize = DISCRIMINATOR_LENGTH // discriminator
        + PUBLIC_KEY_LENGTH // address
        + PUBLIC_KEY_LENGTH // proposal account
        + BOOL_LENGTH // upvote
        + I64_LENGTH // timestamp
        + BOOL_LENGTH; // voted
}

impl<'info> InitializeBuidl<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault.to_account_info(),
            current_authority: self.owner.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Proposal must be for at least 3 days")]
    ProposalTooShort,
    #[msg("Amount too low")]
    AmountTooLow,
    Overflow,
    #[msg("Already voted the same vote on this proposal")]
    AlreadyVoted,
    #[msg("The proposal is ongoing. You can't withdraw yet")]
    ProposalNotOver,
    #[msg("The proposal didn't pass. You can't withdraw")]
    ProposalNotPassed,
}
