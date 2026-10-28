# Outcomes

Game results of the positions.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**black_wins** | **int** | Positions from games Black won. | 
**draws** | **int** | Positions from drawn games. | 
**white_wins** | **int** | Positions from games White won. | 

## Example

```python
from chess_nnue_lab.models.outcomes import Outcomes

# TODO update the JSON string below
json = "{}"
# create an instance of Outcomes from a JSON string
outcomes_instance = Outcomes.from_json(json)
# print the JSON string representation of the object
print(Outcomes.to_json())

# convert the object into a dict
outcomes_dict = outcomes_instance.to_dict()
# create an instance of Outcomes from a dict
outcomes_from_dict = Outcomes.from_dict(outcomes_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


