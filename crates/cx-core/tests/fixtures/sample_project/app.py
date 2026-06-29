"""A tiny sample application module."""

import math


class Account:
    """A bank account with a running balance."""

    def __init__(self, owner: str, balance: float = 0.0):
        self.owner = owner
        self.balance = balance

    def deposit(self, amount: float) -> float:
        # Adds amount to the balance and returns the new balance.
        self.balance += amount
        return self.balance

    def withdraw(self, amount: float) -> float:
        if amount > self.balance:
            raise ValueError("insufficient funds")
        self.balance -= amount
        return self.balance


def compound(principal: float, rate: float, years: int) -> float:
    """Return the compounded value."""
    return principal * math.pow(1 + rate, years)
