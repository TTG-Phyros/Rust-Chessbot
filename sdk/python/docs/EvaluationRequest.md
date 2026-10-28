# EvaluationRequest

An evaluation match, as a client asks for it.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**baseline** | **UUID** | The model it plays against (default: the classical evaluation). | [optional] 
**candidate** | **UUID** | The model evaluated. | 
**games** | **int** | Games to play at most, 2 to 100000 (rounded up to pairs). | [optional] 
**move_time_ms** | **int** | Milliseconds per move, 10 to 60000, instead of nodes: a slower network then searches less. Not reproducible. | [optional] 
**name** | **str** | Name (default: from the contestants). | [optional] 
**nodes** | **int** | Nodes searched per move, 100 to 10000000 (default 5000 unless &#x60;moveTimeMs&#x60; is given). Matches at fixed nodes are reproducible. | [optional] 
**seed** | **int** | Seed of the openings (default: drawn at random, then recorded). | [optional] 
**sprt** | [**Sprt**](Sprt.md) | Stop as soon as this test concludes (default: play every game). | [optional] 
**threads** | **int** | Worker threads (0: all cores). | [optional] 

## Example

```python
from chess_nnue_lab.models.evaluation_request import EvaluationRequest

# TODO update the JSON string below
json = "{}"
# create an instance of EvaluationRequest from a JSON string
evaluation_request_instance = EvaluationRequest.from_json(json)
# print the JSON string representation of the object
print(EvaluationRequest.to_json())

# convert the object into a dict
evaluation_request_dict = evaluation_request_instance.to_dict()
# create an instance of EvaluationRequest from a dict
evaluation_request_from_dict = EvaluationRequest.from_dict(evaluation_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


