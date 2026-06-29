"""A small order-processing module used to demonstrate squeezing."""

from dataclasses import dataclass, field


@dataclass
class LineItem:
    """A single line on an order."""

    sku: str
    quantity: int
    unit_price: float

    def subtotal(self) -> float:
        # Quantity times unit price.
        return self.quantity * self.unit_price


@dataclass
class Order:
    """A customer order made up of line items."""

    customer: str
    items: list[LineItem] = field(default_factory=list)

    def add(self, item: LineItem) -> None:
        self.items.append(item)

    def total(self) -> float:
        """Sum the subtotals of every line item."""
        running = 0.0
        for item in self.items:
            running += item.subtotal()
        return running

    def is_empty(self) -> bool:
        return len(self.items) == 0


def discounted_total(order: Order, rate: float) -> float:
    """Apply a flat discount rate to an order total."""
    return order.total() * (1.0 - rate)
