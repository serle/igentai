# Concurrent LLM Orchestration for Exhaustive Topic Exploration

**PLEASE DO NOT DISTRIBUTE**

November 8, 2024  
iGent AI Backend Interview

This task involves three main components:

a) Setting up a simple distributed system that orchestrates producers and consumers, and manages inter-process communication

b) Calling **LLM APIs** and prompting them effectively

c) Real-time metric calculation and logging via a simple web interface.

While we will be evaluating your design decisions, approach and coding style, we encourage and strongly advise you to use **AI** coding tools to help you achieve the task in the time available.

## 1. Your Task

The high-level goal is to get **LLMs** to exhaustively explore a given topic, generating a list of unique attributes that is as long as possible. For example, if the input topic is "Paris attractions", valid attributes might include "Eiffel tower" and "Louvre". Alternatively if the topic is simply "proper nouns", then examples of valid attributes are "Mars", "Shakespeare" and "Silicon Valley".

Your task is to write a program that takes in a topic as input, and produces a long list of unique strings pertaining to that topic as output, appending them to a file, and showing live metric updates in a web **UI**. To ensure uniqueness, each producer (and **LLM** generation) must be informed of all previously generated attributes in its context¹.

```
                    ┌─────────┐
                    │  LLM 1  │
                    └────┬────┘
                         │
┌─────────┐         ┌────▼────┐         ┌─────────┐
│  LLM 2  ├────────►│         │◄────────┤ Web UI  │
└─────────┘         │Orchestra│         └─────────┘
                    │   tor   │
┌─────────┐         │         │         ┌──────────┐
│  LLM N  ├────────►│         ├────────►│Attribute │
└─────────┘         └─────────┘         │  File    │
                                        └──────────┘
```
*Figure 1: N concurrent LLM producers orchestrated by a central consumer.*

¹ For very long lists of generations, this may exceed the LLM's context length. While we do not expect you to do this, a valid approach might be to partition the attributes in some way. For instance, with the Paris attractions example, you might partition things into culinary activities, museum activities, and so forth; thus splitting the list up into topics, and reducing the number of tokens passed to the input context.

Note that the efficiency of the system will depend on how frequently the list of current generations is communicated to each **LLM** producer, thus helping to avoid duplicated generations.

## 2. Implementation Requirements

Here are some requirements for the three major components of this task that you should consider carefully. Please use a language that you think would be suitable for the task and that you are comfortable writing in. We recommend using Python or Golang.

### 2.1 Distributed System Orchestration

The program should follow the structure described in Figure 1. It should consist of a central orchestration process that manages n **LLM** producer processes running concurrently (where n ≈ 5 and is configurable). You should set up communication channels between the orchestrator and the producers to communicate the initial topic, the list of unique attributes produced so far², and potentially control signals too such as stop signals.

In addition, the central orchestrator should be continuously appending to a file which contains the newline-delimited list of generated strings, as well as providing live updates to we web **UI** displaying certain metrics, described in Section 2.3 below.

² You might also choose to explore all-to-all communication patterns to reduce the time between unique list updates in all the workers.

### 2.2 LLM API Calling and Prompting

We're interested in seeing how you work with **LLM APIs**. This doesn't require any knowledge of transformers or advanced **ML**, but it will involve some simple **API** requests made through the different providers' **SDKs**. You should have received some ephemeral **API** keys from providers such as OpenAI and Anthropic—please implement **LLM** inference for each of these.

The n **LLM** producers depicted in Figure 1 should follow this sequence of steps:

1. Setup the **LLM** client for the provider (e.g. Anthropic, OpenAI, etc.)³
2. Get the topic from the producer, and start the generation loop
3. Generate a chat completion to obtain some new, hopefully unique, items
4. Communicate these new completions to the consumer or peer processes
5. Loop back to 3, conditioning the **LLM** on the updated list

³ Initially, each of the n producers should generate completions from different providers. However, if you get on to the bonus tasks in Section 3, a producer may dynamically change client (and provider).

#### Calling LLM APIs

If you are unfamiliar with **LLM** inference endpoints, here is a minimal example of getting text generations in Python. You should read the documentation from each provider for more information, and note that you don't strictly need to use the providers' **SDKs** to make requests; direct **HTTP** requests will also work.

**Example with an OpenAI client in Go:**

The OpenAI go client is at https://github.com/openai/openai-go

```go
package main

import (
    "context"
    
    "github.com/openai/openai-go"
    "github.com/openai/openai-go/option"
)

func main() {
    client := openai.NewClient(
        option.WithAPIKey("My API Key"), // defaults to os.LookupEnv("OPENAI_API_KEY")
    )
    chatCompletion, err := client.Chat.Completions.New(context.TODO(), openai.ChatCompletionNewParams{
        Messages: openai.F([]openai.ChatCompletionMessageParamUnion{
            openai.UserMessage("Say this is a test"),
        }),
        Model: openai.F(openai.ChatModelGPT4o),
    })
    if err != nil {
        panic(err.Error())
    }
    println(chatCompletion.Choices[0].Message.Content)
}
```

**Example in Python:**

```python
import openai
client = openai.OpenAI(api_key=openai_api_key)

sys_prompt = "You are a topic exploration assistant"
user_prompt = # See below
response = client.chat.completions.create(
    model="gpt-4o-mini",
    temperature=0.7, max_tokens=1024,
    messages=[
        dict(role="system", content=sys_prompt),
        dict(role="user", content=user_prompt),
    ],
)
# text response is in:
# response.choices[0].message.content
```

#### Prompt Engineering

Careful prompt engineering will be essential for efficiently generating unique entries. Here is a simple example of how you might construct the `user_prompt` variable in the examples above. This is only intended as a starter and you may get much better results by iterating on it:

```python
# Get 'topic' and 'existing_entries' from the orchestrator
user_prompt = f"""Generate 100 new entries about: {topic}
Only generate canonical names, in English when available. Omit any descriptions of the entries.

Previous entries:
{existing_entries}

Remember:
- Your entries should be entirely unique from the previous
- Entries should be specific
- One entry per line"""
```

#### Result Parsing and Filtering

After receiving the result from the **LLM**, you will have to parse and filter the output to extract new, unique entries. Exactly how and where you decide to do this is up to you: each producer could handle parsing the string response, or it could send the full string to the orchestrator/consumer process to be processed centrally, or some other approach.

The filtering will become more computationally intensive as the list of entries grows, and accurately identifying duplicates may become frustrated by changes in case and slight word changes⁴. Make sure your solution accounts for these issues.

⁴ If you're comfortable with this, you may choose to use an embedding-based similarity measure, or a data structure like a Bloom filter to efficiently identify duplicates

### 2.3 Output and Logging

The third and final component of this task is calculating and logging key metrics of the system's performance.

#### Metrics to Log

The main metrics to report over time are:

• **Total Unique Entries**: The cumulative count of all unique entries generated
• **Entries per Minute**: The number of valid, unique entries generated within the last minute
• **Per-LLM Performance**: Track the number of unique entries generated within the last minute for each LLM

#### File Output

In addition to logging the metrics above, the system should be writing newly generated and unique entries to an output file, the format of which should be a newline-delimited list of strings, in a file called `output.txt`.

#### Web Visualization

Finally, you should set up a simple web interface to display these live metrics as the application runs. Use Maestro's web proxy feature to run this, and remember that only HTTP services running on port 8080 are available.

Here are some suggestions you may want to consider:

• **HTML Page**: Use a simple HTML framework (e.g., Flask or Node.js) to serve a page with live metrics.
• **Real-Time Updates**: Employ WebSockets to update the webpage live with new entries and performance data.
• **Graphical Representation**: Use charting libraries, such as Chart.js, to visualize metrics like entries per minute.

## 3. Additional Information

You will have access to iGent's Maestro agent for this assignment. The agent is capable of helping you to discuss design ideas, write the application code, write the **LLM** provider **API** calls, implement a filtering and uniqueness check mechanism, write the web **UI**, run code, host the web UI and test the system. In the interests of not only completing the task on time, but also writing sophisticated system components, you should make extensive use of Maestro to help you out.

You do however remain responsible for everything the agent writes and all the code submitted. We expect you to be able to explain all the design decisions taken and articulate the tradeoffs with alternatives. More broadly, we will be evaluating your submission with respect to the following considerations:

• The effectiveness of the final solution
• Testing and verification: does everything run correctly?
• The code quality and style: is it consistent and well thought out?

### Bonus: Dynamic Routing

Since we're calculating performance metrics per **LLM**, you might find that after some time, some **LLMs** will emerge as weaker and less coherent than others, resulting in a lower efficiency. You may therefore choose to alter the constitution of the producer pool to make heavier use of the most efficient **LLMs**.

This may involve dynamically allocating more requests to high-performing **LLMs**, or routing more queries to an LLM that produces more unique entries per minute.

**Good luck!**