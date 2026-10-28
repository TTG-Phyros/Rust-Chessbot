# Contestant

A player of an evaluation match: a model, or the classical evaluation.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture_hash** | **str** | Hash of its architecture. | [optional] 
**bytes** | **int** | Size of its network file. | [optional] 
**model** | **UUID** | The model; absent for the classical evaluation. | [optional] 
**name** | **str** | Its name. | 
**parameters** | **int** | Trainable parameters of its network. | [optional] 

## Example

```python
from chess_nnue_lab.models.contestant import Contestant

# TODO update the JSON string below
json = "{}"
# create an instance of Contestant from a JSON string
contestant_instance = Contestant.from_json(json)
# print the JSON string representation of the object
print(Contestant.to_json())

# convert the object into a dict
contestant_dict = contestant_instance.to_dict()
# create an instance of Contestant from a dict
contestant_from_dict = Contestant.from_dict(contestant_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


