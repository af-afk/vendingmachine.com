// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

interface IVendingMachine as IErrors {
    function lockup(address recipient) external payable returns (uint256 ticketNo);

    /**
     * @notice This function is used by Chainlink as a callback.
     */
    function rawFulfillRandomWords(uint256 ticket, uint256[] calldata words) external;
}
