//! Where food and wares change hands, and at what price.
//!
//! Two goods travel: food and wares. Services cannot be carried and reckoning is used up in the
//! act of trading, so those two are settled inside a town. What a town does with a price is
//! decided by one rule about people — **everybody eats first** — and one about everything else,
//! which is that above subsistence income is spent in fixed shares.
//!
//! That is Stone–Geary demand, and it is the smallest model that has Engel's law in it: a
//! household at subsistence spends everything on food, and the richer it gets the smaller the
//! share food takes. A world that grows therefore shifts from farming into making, serving and
//! reckoning *because people want different things once they are fed*, which is what every
//! economy that ever grew actually did and what nothing in this workspace could previously
//! express.
//!
//! ## A market is a tree
//!
//! Local, state, country, world: a town trades with its state's market, a state with its
//! country's, a country with the world's. Every link has a **wedge** — what it costs to carry a
//! good along it and settle for it — so a town that exports wares gets the market's price less
//! the wedge and a town that imports pays the price plus it. Where a town's own price falls
//! inside that band, it does not trade at all.
//!
//! This is the old spatial-equilibrium problem, and on a tree it has a clean solution. Each node
//! has an **autarky price**, where its whole subtree trades nothing with its parent; from the
//! bottom up every node knows its own, and from the top down every node's price follows from
//! its parent's and its wedge. The whole thing is a few thousand evaluations of a closed form.
//!
//! Nothing about the tree is authored here. Who is whose parent is decided elsewhere, by where
//! trade actually goes; this module only clears what it is handed.

/// How income above subsistence is spent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tastes {
    /// On eating better than subsistence.
    pub food: f64,
    /// On wares to use.
    pub wares: f64,
    /// On services.
    pub services: f64,
    /// Put by, as wares that become capital.
    pub saving: f64,
}

impl Tastes {
    /// Shares that sum to one. Once fed, people spend about a tenth more on food, a third on
    /// things, two fifths on being served, and put by the rest.
    ///
    /// These are the long-run expenditure shares of an economy past subsistence — food falling
    /// towards a tenth, services rising past two fifths — and the saving rate of an agrarian
    /// economy that invests in its tools, not of a modern one.
    pub const ORDINARY: Tastes = Tastes {
        food: 0.12,
        wares: 0.33,
        services: 0.40,
        saving: 0.15,
    };

    /// Of income above subsistence *from tradables*, the shares that go on food and on wares.
    ///
    /// Services are paid for out of the same income, but by the town's own residents to other
    /// residents, so from outside the town the services share simply scales up everything else.
    pub fn tradable(&self) -> (f64, f64) {
        let rest = (1.0 - self.services).max(1e-9);
        (self.food / rest, (self.wares + self.saving) / rest)
    }
}

/// A town as its market sees it: what it made of the two goods that travel, and how many it
/// has to feed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offer {
    pub food: f64,
    pub wares: f64,
    pub mouths: f64,
}

/// What a town does at a price: how many wares it sells (negative: buys).
///
/// `q` is the price of a ware in years of food. Everybody eats first — if the town's whole
/// income cannot buy its subsistence it spends all of it on food and sells every ware it made —
/// and above that the tradable shares decide.
pub fn excess_wares(offer: &Offer, q: f64, tastes: &Tastes) -> f64 {
    let income = offer.food + q * offer.wares;
    let need = offer.mouths;
    if income <= need {
        return offer.wares;
    }
    let (_, wares_share) = tastes.tradable();
    offer.wares - wares_share * (income - need) / q
}

/// Where a town's income went at a price.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spending {
    /// Food eaten, in years of food.
    pub food: f64,
    /// Wares used up, and wares put by as capital.
    pub wares: f64,
    pub invested: f64,
    /// Services bought, valued in food.
    pub services: f64,
    /// How far short of subsistence, as a share of it. Zero when everybody ate.
    pub hunger: f64,
}

/// How a town spends at its local price, after it has traded.
pub fn spend(offer: &Offer, q: f64, tastes: &Tastes) -> Spending {
    let income = offer.food + q * offer.wares;
    let need = offer.mouths;
    if income <= need || need <= 0.0 {
        return Spending {
            food: income.max(0.0),
            hunger: if need > 0.0 {
                (1.0 - income / need).clamp(0.0, 1.0)
            } else {
                0.0
            },
            ..Spending::default()
        };
    }
    let spare = income - need;
    let (food_share, wares_share) = tastes.tradable();
    let wares = wares_share * spare / q;
    let to_capital = tastes.saving / (tastes.wares + tastes.saving).max(1e-9);
    Spending {
        food: need + food_share * spare,
        wares: wares * (1.0 - to_capital),
        invested: wares * to_capital,
        // What residents pay each other for services, which is income to the ones serving.
        services: tastes.services * spare / (1.0 - tastes.services).max(1e-9),
        hunger: 0.0,
    }
}

const Q_MIN: f64 = 1e-9;
const Q_MAX: f64 = 1e9;

/// The markets, as a tree over towns.
///
/// Nodes `0..towns` are the towns themselves; every other node is a market. Each node but the
/// root has a parent and a wedge on the link to it.
#[derive(Clone, Debug)]
pub struct Tree {
    parent: Vec<Option<usize>>,
    wedge: Vec<f64>,
    children: Vec<Vec<usize>>,
    towns: usize,
    root: usize,
}

/// What clearing found.
#[derive(Clone, Debug)]
pub struct Clearing {
    /// The price of a ware in food at every node.
    pub price: Vec<f64>,
    /// Wares each node ships up to its parent; negative is wares coming down.
    pub shipped: Vec<f64>,
    /// Food lost to wedges on every link, in years of food.
    pub freight: f64,
}

impl Tree {
    /// `parent[i]` is node `i`'s parent (the root has none) and `wedge[i]` the cost of the link
    /// to it, as a share of the price. Towns are nodes `0..towns`.
    pub fn new(parent: Vec<Option<usize>>, wedge: Vec<f64>, towns: usize) -> Tree {
        assert_eq!(parent.len(), wedge.len());
        let n = parent.len();
        let mut children = vec![Vec::new(); n];
        let mut root = None;
        for (node, up) in parent.iter().enumerate() {
            match up {
                Some(p) => children[*p].push(node),
                None => {
                    assert!(root.is_none(), "a market has one top");
                    root = Some(node);
                }
            }
        }
        Tree {
            parent,
            wedge: wedge.into_iter().map(|w| w.clamp(0.0, 0.95)).collect(),
            children,
            towns,
            root: root.expect("a market has a top"),
        }
    }

    pub fn root(&self) -> usize {
        self.root
    }

    pub fn parent(&self, node: usize) -> Option<usize> {
        self.parent[node]
    }

    pub fn children(&self, node: usize) -> &[usize] {
        &self.children[node]
    }

    /// What a node's whole subtree sells when the node's own price is `q`.
    fn subtree(&self, node: usize, q: f64, offers: &[Offer], tastes: &Tastes, autarky: &[f64]) -> f64 {
        if node < self.towns {
            return excess_wares(&offers[node], q, tastes);
        }
        self.children[node]
            .iter()
            .map(|child| self.link(*child, q, offers, tastes, autarky))
            .sum()
    }

    /// The price a child faces given its parent's: the band edge it trades at, or its own
    /// autarky price if that lies inside the band.
    fn price_below(&self, child: usize, q: f64, autarky: &[f64]) -> f64 {
        let (low, high) = (q * (1.0 - self.wedge[child]), q * (1.0 + self.wedge[child]));
        autarky[child].clamp(low, high)
    }

    fn link(&self, child: usize, q: f64, offers: &[Offer], tastes: &Tastes, autarky: &[f64]) -> f64 {
        let (low, high) = (q * (1.0 - self.wedge[child]), q * (1.0 + self.wedge[child]));
        let a = autarky[child];
        if a < low {
            self.subtree(child, low, offers, tastes, autarky)
        } else if a > high {
            self.subtree(child, high, offers, tastes, autarky)
        } else {
            0.0
        }
    }

    /// Where a node trades nothing with its parent.
    fn autarky_of(&self, node: usize, offers: &[Offer], tastes: &Tastes, autarky: &[f64]) -> f64 {
        let at = |q: f64| self.subtree(node, q, offers, tastes, autarky);
        let (mut lo, mut hi) = (Q_MIN.ln(), Q_MAX.ln());
        let (at_lo, at_hi) = (at(Q_MIN), at(Q_MAX));
        if at_lo >= 0.0 && at_hi >= 0.0 {
            // Wares are in excess at any price: the subtree cannot feed itself and will sell
            // everything it makes for food. Its price is the floor.
            return if at_lo == 0.0 && at_hi == 0.0 { 1.0 } else { Q_MIN };
        }
        if at_lo <= 0.0 && at_hi <= 0.0 {
            // Wares are short at any price: it makes none and wants some.
            return Q_MAX;
        }
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if at(mid.exp()) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        (0.5 * (lo + hi)).exp()
    }

    /// Clear every market at once.
    pub fn clear(&self, offers: &[Offer], tastes: &Tastes) -> Clearing {
        assert_eq!(offers.len(), self.towns);
        let n = self.parent.len();
        // Bottom up: every node's autarky price, children before parents.
        let order = self.bottom_up();
        let mut autarky = vec![1.0; n];
        for node in &order {
            autarky[*node] = self.autarky_of(*node, offers, tastes, &autarky);
        }
        // Top down: the root trades with nobody, and every child's price follows its parent's.
        let mut price = vec![1.0; n];
        price[self.root] = autarky[self.root];
        for node in order.iter().rev() {
            if let Some(up) = self.parent[*node] {
                price[*node] = self.price_below(*node, price[up], &autarky);
            }
        }
        let mut shipped = vec![0.0; n];
        let mut freight = 0.0;
        for node in 0..n {
            let Some(up) = self.parent[node] else {
                continue;
            };
            let sold = self.subtree(node, price[node], offers, tastes, &autarky);
            // A node inside its band trades nothing, whatever its subtree does at that price.
            let trades = (price[node] - autarky[node]).abs() > 1e-12 * price[node].max(1e-12);
            shipped[node] = if trades { sold } else { 0.0 };
            freight += shipped[node].abs() * price[up] * self.wedge[node];
        }
        Clearing {
            price,
            shipped,
            freight,
        }
    }

    fn bottom_up(&self) -> Vec<usize> {
        let mut order = Vec::with_capacity(self.parent.len());
        let mut stack = vec![(self.root, false)];
        while let Some((node, expanded)) = stack.pop() {
            if expanded {
                order.push(node);
            } else {
                stack.push((node, true));
                for child in &self.children[node] {
                    stack.push((*child, false));
                }
            }
        }
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TASTES: Tastes = Tastes::ORDINARY;

    fn farm(mouths: f64) -> Offer {
        Offer {
            food: 1.6 * mouths,
            wares: 0.05 * mouths,
            mouths,
        }
    }

    fn forge(mouths: f64) -> Offer {
        Offer {
            food: 0.6 * mouths,
            wares: 1.2 * mouths,
            mouths,
        }
    }

    /// Everybody eats first: a town that cannot afford its bread buys no wares at all.
    #[test]
    fn nobody_buys_things_before_bread() {
        let hungry = Offer {
            food: 50.0,
            wares: 10.0,
            mouths: 100.0,
        };
        assert_eq!(excess_wares(&hungry, 2.0, &TASTES), 10.0);
        let spent = spend(&hungry, 2.0, &TASTES);
        assert!((spent.food - 70.0).abs() < 1e-9);
        assert!((spent.hunger - 0.3).abs() < 1e-9);
        assert_eq!(spent.wares, 0.0);
    }

    /// Engel's law: the richer a town, the smaller the share of its income that goes on food.
    #[test]
    fn the_richer_a_town_the_less_of_it_goes_on_food() {
        let mut share = f64::MAX;
        for rich in [1.1, 1.5, 3.0, 8.0, 30.0] {
            let offer = Offer {
                food: rich * 100.0,
                wares: 0.0,
                mouths: 100.0,
            };
            let spent = spend(&offer, 1.0, &TASTES);
            let total = spent.food + spent.wares + spent.invested + spent.services;
            let food_share = spent.food / total;
            assert!(food_share < share, "at {rich}× subsistence food took {food_share:.3}");
            share = food_share;
        }
        assert!(share < 0.2);
    }

    /// Two towns that differ trade; the farm sends food and the forge sends wares, and the
    /// price settles between their two autarky prices.
    #[test]
    fn two_towns_that_differ_trade_and_two_that_do_not_do_not() {
        let tree = Tree::new(vec![Some(2), Some(2), None], vec![0.05, 0.05, 0.0], 2);
        let clearing = tree.clear(&[farm(100.0), forge(100.0)], &TASTES);
        assert!(clearing.shipped[0] < 0.0, "the farm buys wares");
        assert!(clearing.shipped[1] > 0.0, "the forge sells them");
        assert!(
            (clearing.shipped[0] + clearing.shipped[1]).abs() < 1e-6,
            "what is sold is bought"
        );
        assert!(clearing.freight > 0.0);

        let same = tree.clear(&[farm(100.0), farm(100.0)], &TASTES);
        assert!(same.shipped.iter().all(|s| s.abs() < 1e-9), "{:?}", same.shipped);
    }

    /// A wide enough wedge stops trade that a narrow one allows — distance is what keeps
    /// markets apart.
    ///
    /// Between two towns that can each feed themselves. A town that cannot is different, and
    /// the difference is the point of eating first: cut off by distance, it sells every ware it
    /// has for whatever food they fetch, and ships *more* the further the food has to come.
    #[test]
    fn distance_can_close_a_market() {
        let near = Tree::new(vec![Some(2), Some(2), None], vec![0.05, 0.05, 0.0], 2);
        let far = Tree::new(vec![Some(2), Some(2), None], vec![0.9, 0.9, 0.0], 2);
        let workshop = Offer {
            food: 1.1 * 100.0,
            wares: 1.2 * 100.0,
            mouths: 100.0,
        };
        let a = near.clear(&[farm(100.0), workshop], &TASTES);
        let b = far.clear(&[farm(100.0), workshop], &TASTES);
        assert!(a.shipped[1] > 0.0);
        assert!(b.shipped[1].abs() < a.shipped[1] * 0.5, "{} against {}", b.shipped[1], a.shipped[1]);

        let desperate = far.clear(&[farm(100.0), forge(100.0)], &TASTES);
        assert!(
            desperate.shipped[1] >= a.shipped[1],
            "a hungry town far from food sells everything it has"
        );
    }

    /// Three levels: two states in one country, each with a farm and a forge. Most trade is
    /// inside each state; what crosses the country is the remainder.
    #[test]
    fn a_state_trades_inside_itself_before_it_trades_out() {
        // Towns 0..4; states 4 and 5; country 6.
        let parent = vec![Some(4), Some(4), Some(5), Some(5), Some(6), Some(6), None];
        let wedge = vec![0.05, 0.05, 0.05, 0.05, 0.3, 0.3, 0.0];
        let tree = Tree::new(parent, wedge, 4);
        let offers = [farm(100.0), forge(100.0), farm(120.0), forge(60.0)];
        let clearing = tree.clear(&offers, &TASTES);
        let inside: f64 = clearing.shipped[..4].iter().map(|s| s.abs()).sum();
        let across: f64 = clearing.shipped[4..6].iter().map(|s| s.abs()).sum();
        assert!(inside > across, "{inside} inside the states, {across} between them");
        // And every market balances.
        let total: f64 = clearing.shipped[4..6].iter().sum();
        assert!(total.abs() < 1e-6, "the country clears: {total}");
        for state in [4usize, 5] {
            let net: f64 = tree.children(state).iter().map(|c| clearing.shipped[*c]).sum();
            assert!(
                (net - clearing.shipped[state]).abs() < 1e-6,
                "state {state} passes up exactly its towns' net"
            );
        }
    }

    /// A region that cannot feed itself sells everything for food, and wares go cheap.
    #[test]
    fn famine_makes_wares_cheap() {
        let tree = Tree::new(vec![Some(2), Some(2), None], vec![0.05, 0.05, 0.0], 2);
        let starving = [
            Offer {
                food: 50.0,
                wares: 30.0,
                mouths: 100.0,
            },
            Offer {
                food: 60.0,
                wares: 30.0,
                mouths: 100.0,
            },
        ];
        let fed = [farm(100.0), forge(100.0)];
        let lean = tree.clear(&starving, &TASTES);
        let full = tree.clear(&fed, &TASTES);
        assert!(lean.price[2] < full.price[2]);
    }
}
