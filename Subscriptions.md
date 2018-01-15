Subscription mechanism:

# Basic concept: Index Nodes as subscription lookup

When creating a subscription, originate an edit to the commensurate index node announcing the creation of the subscription. The subscription may be ephemeral in the form of a slabref, or long-lived in the form of a trigger (not in reference to any slabref). When originating an edit, issue a new index node memo pointing to the new subject head, notice any active subscriptions, and send the edits in question to the appropriate slabrefs.

Triggers are somewhat more clear insofar as they'd entail the lookup of an action, and the (possibly duplicative) dispatch to same. Ephemeral observers are somewhat murkier – one could observer slabrefs individually, but there might be a lot of them. 

# Reflections

1. A temporary, host-only subscription mechanism would acheive what?
It would allow us to test causal context compaction, albeit with manual, or overly chatty memo exchanges. This may be worth doing, even though it would be throwaway code.
2.  We need to figure out a way to make a gossip network of observers, such that the whole list isn't registered, but the network of observers for a given subject is well connected.
3. Re-read and re-grok https://homes.cs.washington.edu/~arvind/papers/pubsub.pdf