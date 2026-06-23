-- Migration: Create PotBonus Relational Database Schema
-- Created: 2026-06-23

-- 1. Global state table (holds single row with pool points)
CREATE TABLE pot_bonus_state (
    id INTEGER PRIMARY KEY DEFAULT 1 CONSTRAINT check_single_row CHECK (id = 1),
    pool_points INTEGER NOT NULL DEFAULT 0 CONSTRAINT check_non_negative_points CHECK (pool_points >= 0),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Initialize global state
INSERT INTO pot_bonus_state (id, pool_points) VALUES (1, 0) ON CONFLICT DO NOTHING;

-- 2. Persistent account registrations mapping account_id -> user_id
CREATE TABLE pot_bonus_registrations (
    account_id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Index to quickly query all accounts owned by a user
CREATE INDEX idx_pot_bonus_registrations_user ON pot_bonus_registrations (user_id);

-- 3. Weekly graduations table (cleared during weekly reset)
CREATE TABLE pot_bonus_weekly_graduations (
    account_id UUID PRIMARY KEY REFERENCES pot_bonus_registrations(account_id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Index to quickly find weekly graduations for a user
CREATE INDEX idx_pot_bonus_weekly_graduations_user ON pot_bonus_weekly_graduations (user_id);

-- 4. Weekly matrix cycles table (cleared during weekly reset)
CREATE TABLE pot_bonus_weekly_cycles (
    account_id UUID NOT NULL REFERENCES pot_bonus_registrations(account_id) ON DELETE CASCADE,
    matrix_id UUID NOT NULL,
    user_id UUID NOT NULL,
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (account_id, matrix_id)
);

-- Index to quickly find weekly cycles for a user
CREATE INDEX idx_pot_bonus_weekly_cycles_user ON pot_bonus_weekly_cycles (user_id);
