# SelfplayRequest

A self-play dataset, as a client asks for it.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**games** | **int** | Games to play. | 
**name** | **str** | Name of the dataset (default: from the parameters). | [optional] 
**network** | **UUID** | Model playing the games (default: the classical evaluation). | [optional] 
**nodes** | **int** | Nodes searched per move. | [optional] 
**seed** | **int** | Seed (default: drawn at random, then recorded). | [optional] 
**threads** | **int** | Worker threads (0: all cores). | [optional] 

## Example

```python
from chess_nnue_lab.models.selfplay_request import SelfplayRequest

# TODO update the JSON string below
json = "{}"
# create an instance of SelfplayRequest from a JSON string
selfplay_request_instance = SelfplayRequest.from_json(json)
# print the JSON string representation of the object
print(SelfplayRequest.to_json())

# convert the object into a dict
selfplay_request_dict = selfplay_request_instance.to_dict()
# create an instance of SelfplayRequest from a dict
selfplay_request_from_dict = SelfplayRequest.from_dict(selfplay_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


