// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface IERC20 {
    function transferFrom(address from, address to, uint256 value) external returns (bool);
    function transfer(address to, uint256 value) external returns (bool);
}

/**
 * @title ImperativeLimitOrderBook
 * @notice Price-time priority limit order book with imperative matching logic
 */
contract ImperativeLimitOrderBook {
    // Storage state
    IERC20 public immutable token0;
    IERC20 public immutable token1;
    
    mapping(address => uint256) public balances0;
    mapping(address => uint256) public balances1;
    mapping(uint256 => Order) public bidOrders;
    mapping(uint256 => Order) public askOrders;
    
    uint256 public firstBidOrder;
    uint256 public firstAskOrder;
    uint256 private orderCounter = 1;
    
    struct Order {
        uint256 maxAmount;
        uint256 price;
        bool isBid;
        address owner;
        uint256 nextKey;
        bool isActive;
    }
    
    struct MarketOrder {
        address user;
        uint256 amount;
    }
    
    event OrderAdded(uint256 indexed id, bool isBid, uint256 price, uint256 amount);
    event OrderRemoved(uint256 indexed id);
    event Trade(uint256 indexed bidId, uint256 indexed askId, uint256 amount, uint256 price);
    
    constructor(address _token0, address _token1) {
        token0 = IERC20(_token0);
        token1 = IERC20(_token1);
    }
    
    // DEPOSIT
    function deposit(uint256 amount0, uint256 amount1) external {
        if (amount0 > 0) {
            require(token0.transferFrom(msg.sender, address(this), amount0), "Token0 transfer failed");
            balances0[msg.sender] += amount0;
        }
        if (amount1 > 0) {
            require(token1.transferFrom(msg.sender, address(this), amount1), "Token1 transfer failed");
            balances1[msg.sender] += amount1;
        }
    }
    
    // WITHDRAW
    function withdraw(uint256 amount0, uint256 amount1) external {
        require(balances0[msg.sender] >= amount0, "Insufficient balance0");
        require(balances1[msg.sender] >= amount1, "Insufficient balance1");
        
        if (amount0 > 0) {
            balances0[msg.sender] -= amount0;
            require(token0.transfer(msg.sender, amount0), "Token0 transfer failed");
        }
        if (amount1 > 0) {
            balances1[msg.sender] -= amount1;
            require(token1.transfer(msg.sender, amount1), "Token1 transfer failed");
        }
    }
    
    // ADD BID ORDER
    function addBid(uint256 price, uint256 amount) external returns (uint256) {
        require(price > 0 && amount > 0, "Invalid parameters");
        uint256 cost = price * amount;
        require(balances1[msg.sender] >= cost, "Insufficient balance");
        balances1[msg.sender] -= cost; // Lock funds
        
        uint256 orderId = orderCounter++;
        
        //LOOP to find insertion position
        uint256 current = firstBidOrder;
        uint256 previous = 0;
        
        while (current != 0) {
            Order storage currentOrder = bidOrders[current];
            
            if (currentOrder.price > price) {
                previous = current;
                current = currentOrder.nextKey;
            } else if (currentOrder.price == price) {
                uint256 lastSamePrice = _findLastOrderAtPrice(current, true);
                previous = lastSamePrice;
                current = bidOrders[lastSamePrice].nextKey;
                break;
            } else {
                break;
            }
            
        }
        
        // Create and insert order
        bidOrders[orderId] = Order({
            maxAmount: amount,
            price: price,
            isBid: true,
            owner: msg.sender,
            nextKey: current,
            isActive: true
        });
        
        // Update linked list
        if (previous == 0) {
            firstBidOrder = orderId;
        } else {
            bidOrders[previous].nextKey = orderId;
        }
        
        emit OrderAdded(orderId, true, price, amount);
        return orderId;
    }
    
    function _findLastOrderAtPrice(uint256 startId, bool isBid) private view returns (uint256) {
        mapping(uint256 => Order) storage orders = isBid ? bidOrders : askOrders;
        Order storage startOrder = orders[startId];
        
        if (startOrder.nextKey == 0 || orders[startOrder.nextKey].price != startOrder.price) {
            return startId;
        }
        
        return _findLastOrderAtPrice(startOrder.nextKey, isBid);
    }
    
    // ADD ASK ORDER
    function addAsk(uint256 price, uint256 amount) external returns (uint256) {
        require(price > 0 && amount > 0, "Invalid parameters");
        require(balances0[msg.sender] >= amount, "Insufficient balance");
        balances0[msg.sender] -= amount; // Lock funds
        
        uint256 orderId = orderCounter++;
        
        uint256 current = firstAskOrder;
        uint256 previous = 0;
        
        while (current != 0) {
            Order storage currentOrder = askOrders[current];
            
            if (currentOrder.price < price) {
                previous = current;
                current = currentOrder.nextKey;
            } else if (currentOrder.price == price) {
                uint256 lastSamePrice = _findLastOrderAtPrice(current, false);
                previous = lastSamePrice;
                current = askOrders[lastSamePrice].nextKey;
                break;
            } else {
                break;
            }
        }
        
        askOrders[orderId] = Order({
            maxAmount: amount,
            price: price,
            isBid: false,
            owner: msg.sender,
            nextKey: current,
            isActive: true
        });
        
        if (previous == 0) {
            firstAskOrder = orderId;
        } else {
            askOrders[previous].nextKey = orderId;
        }
        
        emit OrderAdded(orderId, false, price, amount);
        return orderId;
    }
    
    // CANCEL ORDER
    function cancelOrder(uint256 orderId, bool isBid) external {
        if (isBid) {
            Order storage order = bidOrders[orderId];
            require(order.owner == msg.sender, "Not authorized");
            
            _removeOrderFromList(orderId, true);
            
            // Refund locked funds
            balances1[msg.sender] += order.price * order.maxAmount;
            delete bidOrders[orderId];
        } else {
            Order storage order = askOrders[orderId];
            require(order.owner == msg.sender, "Not authorized");
            
            _removeOrderFromList(orderId, false);
            
            // Refund locked funds
            balances0[msg.sender] += order.maxAmount;
            delete askOrders[orderId];
        }
        
        emit OrderRemoved(orderId);
    }
    
    function _removeOrderFromList(uint256 orderId, bool isBid) private {
        mapping(uint256 => Order) storage orders = isBid ? bidOrders : askOrders;
        uint256 firstOrder = isBid ? firstBidOrder : firstAskOrder;
        
        if (firstOrder == orderId) {
            if (isBid) {
                firstBidOrder = orders[orderId].nextKey;
            } else {
                firstAskOrder = orders[orderId].nextKey;
            }
            return;
        }
        
        uint256 current = firstOrder;
        while (current != 0) {
            if (orders[current].nextKey == orderId) {
                orders[current].nextKey = orders[orderId].nextKey;
                return;
            }
            current = orders[current].nextKey;
            
        }
        
        revert("Order not found");
    }
    
    // SETTLE LIMIT ORDERS
    function settleLimitOrders(uint256 maxMatches) external {
        uint256 matches = 0;
        
        while (firstBidOrder != 0 && firstAskOrder != 0 && matches < maxMatches) {
            Order storage topBid = bidOrders[firstBidOrder];
            Order storage topAsk = askOrders[firstAskOrder];
            
            if (topBid.price < topAsk.price) {
                break; // No crossing orders
            }
            
            // Calculate trade amount
            uint256 tradeAmount = topBid.maxAmount < topAsk.maxAmount ? 
                                  topBid.maxAmount : topAsk.maxAmount;
            uint256 tradePrice = topAsk.price; // Trade at ask price
            
            balances0[topBid.owner] += tradeAmount;
            balances1[topAsk.owner] += tradeAmount * tradePrice;
            
            // Update order amounts
            topBid.maxAmount -= tradeAmount;
            topAsk.maxAmount -= tradeAmount;
            
            emit Trade(firstBidOrder, firstAskOrder, tradeAmount, tradePrice);
            
            if (topBid.maxAmount == 0) {
                _popBestBid();
            }
            if (topAsk.maxAmount == 0) {
                _popBestAsk();
            }
            
            matches++;
            
        }
    }
    
    // INTERNAL HELPER FUNCTIONS
    function _popBestBid() private {
        uint256 orderId = firstBidOrder;
        firstBidOrder = bidOrders[orderId].nextKey;
        delete bidOrders[orderId];
    }
    
    function _popBestAsk() private {
        uint256 orderId = firstAskOrder;
        firstAskOrder = askOrders[orderId].nextKey;
        delete askOrders[orderId];
    }
    
    // MARKET ORDER EXECUTION
    function executeMarketOrders(
        MarketOrder[] calldata bidMarketOrders,
        MarketOrder[] calldata askMarketOrders
    ) external {
        
        // Process bid market orders against ask orderbook
        for (uint256 i = 0; i < bidMarketOrders.length; i++) {
            MarketOrder memory marketOrder = bidMarketOrders[i];
            uint256 remainingAmount = marketOrder.amount;
            
            // match against multiple limit orders
            while (remainingAmount > 0 && firstAskOrder != 0) {
                Order storage topAsk = askOrders[firstAskOrder];
                
                uint256 adjustedPrice = topAsk.price;
                uint256 cost = remainingAmount * adjustedPrice;
                
                require(balances1[marketOrder.user] >= cost, "Insufficient balance for market order");
                
                uint256 fillAmount = remainingAmount < topAsk.maxAmount ? 
                                   remainingAmount : topAsk.maxAmount;
                
                // Execute trade
                balances1[marketOrder.user] -= fillAmount * adjustedPrice;
                balances0[marketOrder.user] += fillAmount;
                balances1[topAsk.owner] += fillAmount * adjustedPrice;
                
                topAsk.maxAmount -= fillAmount;
                remainingAmount -= fillAmount;
                
                if (topAsk.maxAmount == 0) {
                    _popBestAsk();
                }
                
            }
        }
        
        _processAskMarketOrders(askMarketOrders, 0);
    }
    
    // HELPER FUNCTION for market order processing
    function _processAskMarketOrders(
        MarketOrder[] calldata askMarketOrders,
        uint256 index
    ) private {
        // Base case
        if (index >= askMarketOrders.length) {
            return;
        }
        
        MarketOrder memory marketOrder = askMarketOrders[index];
        uint256 remainingAmount = marketOrder.amount;
        
        // Process against bid orderbook
        while (remainingAmount > 0 && firstBidOrder != 0) {
            Order storage topBid = bidOrders[firstBidOrder];
            uint256 adjustedPrice = topBid.price;
            
            require(balances0[marketOrder.user] >= remainingAmount, "Insufficient token0");
            
            uint256 fillAmount = remainingAmount < topBid.maxAmount ? 
                               remainingAmount : topBid.maxAmount;
            
            balances0[marketOrder.user] -= fillAmount;
            balances1[marketOrder.user] += fillAmount * adjustedPrice;
            balances0[topBid.owner] += fillAmount;
            
            topBid.maxAmount -= fillAmount;
            remainingAmount -= fillAmount;
            
            if (topBid.maxAmount == 0) {
                _popBestBid();
            }
            
        }
        
        _processAskMarketOrders(askMarketOrders, index + 1);
    }
    
}