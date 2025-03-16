// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

/* ~~~~~~ EVENTS ~~~~~~ */

interface IEvents {
    event LockedUpTokens(
        address indexed recipient,
        uint256 indexed amount
    );

    event RandomnessRequested(uint256 indexed ticketNo);
}