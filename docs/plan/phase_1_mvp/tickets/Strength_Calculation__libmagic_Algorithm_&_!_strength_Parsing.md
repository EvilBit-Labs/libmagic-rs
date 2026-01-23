# Strength Calculation: libmagic Algorithm & !:strength Parsing

## Overview

Implement libmagic's `apprentice_magic_strength` algorithm with `!:strength` modifier parsing to achieve high GNU file compatibility. Strength affects rule priority and matching behavior, critical for 95%+ test corpus compatibility.

## Scope

**In Scope:**
- Port `apprentice_magic_strength` algorithm from libmagic
- Parse `!:strength` modifiers from magic files
- Calculate default strength based on rule specificity
- Store strength in `MagicRule` structure
- Use strength for rule ordering and confidence scoring

**Out of Scope:**
- Advanced strength heuristics
- Machine learning-based strength
- Performance optimization

## Technical Approach

### 1. Strength Modifier Parsing

Update `file:src/parser/ast.rs`:

```rust
pub enum StrengthModifier {
    Add(i32),      // !:strength +10
    Subtract(i32), // !:strength -5
    Multiply(i32), // !:strength *2
    Divide(i32),   // !:strength /2
    Set(i32),      // !:strength =50
}

pub struct MagicRule {
    // ... existing fields ...
    pub strength_modifier: Option<StrengthModifier>,
}
```

### 2. Strength Parsing

Add to `file:src/parser/grammar.rs`:

```rust
pub fn parse_strength_modifier(line: &str) -> Option<StrengthModifier> {
    // Parse !:strength OP VALUE syntax
    // Examples:
    //   !:strength +10
    //   !:strength -5
    //   !:strength *2
    //   !:strength /2
    //   !:strength =50
}
```

### 3. Default Strength Calculation

Add to `file:src/parser/mod.rs`:

```rust
pub fn calculate_default_strength(rule: &MagicRule) -> i32 {
    // Port libmagic's apprentice_magic_strength algorithm
    // Factors:
    // - Type specificity (string > byte)
    // - Operator specificity (= > &)
    // - Offset type (absolute > indirect)
    // - Value length (longer strings = higher strength)
    
    let mut strength = 0;
    
    // Type contribution
    strength += match rule.type_kind {
        TypeKind::String => 20,
        TypeKind::Long => 15,
        TypeKind::Short => 10,
        TypeKind::Byte => 5,
        _ => 10,
    };
    
    // Operator contribution
    strength += match rule.operator {
        Operator::Equal => 10,
        Operator::NotEqual => 5,
        Operator::And => 3,
        _ => 5,
    };
    
    // Offset contribution
    strength += match rule.offset {
        OffsetSpec::Absolute(_) => 10,
        OffsetSpec::Indirect { .. } => 5,
        OffsetSpec::Relative(_) => 3,
    };
    
    // Value length contribution (for strings)
    if let TypeKind::String = rule.type_kind {
        strength += (rule.value.len() as i32).min(20);
    }
    
    strength
}
```

### 4. Strength Application

Update `file:src/parser/mod.rs`:

```rust
pub fn apply_strength_modifier(base_strength: i32, modifier: &StrengthModifier) -> i32 {
    match modifier {
        StrengthModifier::Add(n) => base_strength + n,
        StrengthModifier::Subtract(n) => base_strength - n,
        StrengthModifier::Multiply(n) => base_strength * n,
        StrengthModifier::Divide(n) => base_strength / n,
        StrengthModifier::Set(n) => *n,
    }
}
```

### 5. Integration with Evaluation

Update `file:src/evaluator/mod.rs`:

```rust
// Use strength for rule ordering
pub fn sort_rules_by_strength(rules: &mut [MagicRule]) {
    rules.sort_by(|a, b| {
        let strength_a = calculate_default_strength(a);
        let strength_b = calculate_default_strength(b);
        strength_b.cmp(&strength_a) // Higher strength first
    });
}
```

## Acceptance Criteria

- [ ] `!:strength` modifiers parsed correctly
- [ ] Default strength calculated for all rule types
- [ ] Strength modifiers applied correctly
- [ ] Rules sorted by strength during evaluation
- [ ] Strength affects confidence scoring
- [ ] Unit tests for strength parsing
- [ ] Unit tests for default strength calculation
- [ ] Unit tests for strength modifiers
- [ ] Integration test comparing with GNU file output
- [ ] Compatibility test shows improved match rate

## Dependencies

- Depends on: ticket:75a688c2-0ac4-489a-a35d-6e824c94c153/c554e409-ae60-407f-9596-64c5b03a9b92 (Parser Integration)

## Related Specs

- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/269e848a-258d-4cd4-99b1-386bd400a109 (Technical Plan - Strength Calculation)
- spec:75a688c2-0ac4-489a-a35d-6e824c94c153/3ce0475b-153d-487f-bc0d-47d0a8f6708a (Epic Brief - 95%+ compatibility target)

## Files to Modify

- `file:src/parser/ast.rs` - Add StrengthModifier enum, update MagicRule
- `file:src/parser/grammar.rs` - Add strength parsing
- `file:src/parser/mod.rs` - Add strength calculation functions
- `file:src/evaluator/mod.rs` - Use strength for rule ordering
- `file:tests/` - Add strength calculation tests
