# ComparedPlayer

A player of a comparison.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**contestant** | [**Contestant**](Contestant.md) | Who it is: name, architecture, network size. | 
**elo** | **float** | Elo relative to the first player, fitted over the games of every pair; absent when no games link it to the first player. | [optional] 
**games** | **int** | Games it played against the other players. | 
**nps** | **int** | Nodes per second measured by its latest evaluation (on that evaluation&#39;s worker: compare speeds measured on the same machine). | [optional] 

## Example

```python
from chess_nnue_lab.models.compared_player import ComparedPlayer

# TODO update the JSON string below
json = "{}"
# create an instance of ComparedPlayer from a JSON string
compared_player_instance = ComparedPlayer.from_json(json)
# print the JSON string representation of the object
print(ComparedPlayer.to_json())

# convert the object into a dict
compared_player_dict = compared_player_instance.to_dict()
# create an instance of ComparedPlayer from a dict
compared_player_from_dict = ComparedPlayer.from_dict(compared_player_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


