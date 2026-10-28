# Problem

An error (RFC 9457). `type` identifies the kind of problem (`urn:chess-bot:problem:<name>`); validation problems list every error found in `errors`.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**detail** | **str** | What went wrong with this request. | [optional] 
**errors** | [**List[FieldError]**](FieldError.md) | Every problem of the request body, for validation errors. | [optional] 
**status** | **int** | HTTP status. | 
**title** | **str** | Short summary of the kind of problem. | 
**type** | **str** | Kind of problem, e.g. &#x60;urn:chess-bot:problem:validation-error&#x60;. | 

## Example

```python
from chess_nnue_lab.models.problem import Problem

# TODO update the JSON string below
json = "{}"
# create an instance of Problem from a JSON string
problem_instance = Problem.from_json(json)
# print the JSON string representation of the object
print(Problem.to_json())

# convert the object into a dict
problem_dict = problem_instance.to_dict()
# create an instance of Problem from a dict
problem_from_dict = Problem.from_dict(problem_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


