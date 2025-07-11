# regit, a git branch manager

## goals:

- ease "stacked pr" workflow
- handle "trees of branches"
- handle re-basing
- handle github pr view

## snips

### find closest base branch

    git merge-base --fork-point main
    git log --format="%H %D" 15d813ad707c03c1bf350b6f9616dde32651f808..HEAD
