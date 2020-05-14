import { kline } from './tanglism-kline.js';
import { stroke } from './tanglism-stroke.js';
import { segment } from './tanglism-segment.js';
import { subtrend } from './tanglism-subtrend.js';
import { center } from './tanglism-center.js';
import { metric } from './tanglism-metric.js';

export function draw() {
  var objs = new Set(query().objects);
  // 总添加K线
  objs.add("KLines");
  // 清理画布
  if (!d3.select("#k_lines").empty()) {
    // 如存在则删除
    d3.select("#k_lines").remove();
  }
  var conf = kline.conf();
    // 创建K线图
    var svg = d3.select("#k_container")
      .append("svg")
      .attr("id", "k_lines")
      .attr("width", conf.w)
      .attr("height", conf.h);

  // 按顺序画图
  if (objs.has("Centers")) {
    center.draw();
  }
  if (objs.has("KLines")) {
    kline.draw();
  }
  if (objs.has("Strokes")) {
    stroke.draw();
  }
  if (objs.has("Segments")) {
    segment.draw();
  }
  if (objs.has("SubTrends")) {
    subtrend.draw();
  }
  if (objs.has("MACD")) {
    metric.draw();
  }
}

export function setupWebsocketEvents(ws) {
  ws.onopen = function(e) {
    console.log("Websocket opened");
    if (validate_basic_cfg()) {
        ws.send(JSON.stringify({
            type: "BasicCfg",
            data: basic_cfg()
        }));
    }
    ws.send(JSON.stringify({
        type: "StrokeCfg",
        data: stroke_cfg()
    }));
    ws.send(JSON.stringify({
        type: "MetricsCfg",
        data: ""
    }));
    // 触发事件
    // 基础配置
    $(".basic_trigger").change(function() {
      if (validate_basic_cfg()) {
        ws.send(JSON.stringify({
          type: "BasicCfg",
          data: basic_cfg()
        }));
        send_query(ws);
      }
    });
    // 笔配置
    $(".stroke_trigger").change(function() {
      ws.send(JSON.stringify({
        type: "StrokeCfg",
        data: stroke_cfg()
      }));
      if (validate_basic_cfg()) {
        send_query(ws);
      }
    });
    // 形态配置
    $(".morph_trigger").change(function() {
      if (validate_basic_cfg()) {
        send_query(ws);
      }
    });
  };
  ws.onclose = function(e) {
    console.log("Websocket closed");
  };
  ws.onmessage = function(e) {
    var resp = JSON.parse(e.data);
    if (resp.type === "Error") {
      console.log("Error: " + resp.data);
    }
    if (resp.type === "Data") {
      prepare_data(resp.data);
      draw();
    }
  };
}

function prepare_data(dataset) {
    var changed = false;
    for (var i = 0; i < dataset.length; i++) {
        if (dataset[i].type === "KLines") {
            kline.data(dataset[i].data);
            changed = true;
        } else if (dataset[i].type === "Strokes") {
            stroke.data(dataset[i].data);
            changed = true;
        } else if (dataset[i].type === "Segments") {
            segment.data(dataset[i].data);
            changed = true;
        } else if (dataset[i].type === "SubTrends") {
            subtrend.data(dataset[i].data);
            changed = true;
        } else if (dataset[i].type === "Centers") {
            center.data(dataset[i].data);
            changed = true;
        } else if (dataset[i].type === "MACD") {
            metric.data("DIF", dataset[i].data.dif);
            metric.data("DEA", dataset[i].data.dea);
            metric.data("MACD", dataset[i].data.macd);
            changed = true;
        }
    }
    return changed;
}

export function validate_basic_cfg() {
  var input_stock_code = $("#input_stock_code").val().trim();
  if (!input_stock_code.endsWith("XSHE") && !input_stock_code.endsWith("XSHG")) {
    return false;
  }
  if ($("#input_start_dt").val().length != 10 || $("#input_end_dt").val().length != 10) {
    return false;
  }
  return true;
}

function basic_cfg() {
  var input_stock_code = $("#input_stock_code").val();
  var input_start_dt = $("#input_start_dt").val();
  var input_end_dt = $("#input_end_dt").val();
  var input_tick = $("#input_tick option:selected").text();
  return {
    tick: input_tick,
    code: input_stock_code,
    start_dt: input_start_dt,
    end_dt: input_end_dt,
  };
}

function stroke_cfg() {
  var stroke_logic_indep_k = $("input[name='stroke_logic_indep_k']:checked").val();
  var stroke_logic_gap = $("input[name='stroke_logic_gap']:checked").val();
  var stroke_cfg;
  if (stroke_logic_indep_k === "indep_k") {
    stroke_cfg = "indep_k";
  } else {
    stroke_cfg = "indep_k:false";
  }
  if (stroke_logic_gap === "gap_opening") {
    stroke_cfg += ",gap_opening";
  } else if (stroke_logic_gap === "gap_ratio") {
    var gap_ratio = parseFloat($("#gap_ratio_percentage").val()) / 100;
    stroke_cfg += ",gap_ratio=" + gap_ratio;
  }
  return stroke_cfg;
}

function query() {
  var objects = [];
  var requires = [];
  if ($("#stroke_draw").is(":checked")) {
    objects.push("Strokes");
    if (stroke.data().length === 0) {
      requires.push("Strokes");
    }
  }
  if ($("#segment_draw").is(":checked")) {
    objects.push("Segments");
    if (segment.data().length === 0) {
      requires.push("Segments");
    }
  }
  if ($("#subtrend_draw").is(":checked")) {
    objects.push("SubTrends");
    if (subtrend.data().length === 0) {
      requires.push("SubTrends");
    }
  }
  if ($("#center_draw").is(":checked")) {
    objects.push("Centers");
    if (center.data().length === 0) {
      requires.push("Centers");
    }
  }
  // 默认画出MACD
  objects.push("MACD");
  if (metric.data("MACD").length === 0) {
    requires.push("MACD");
  }
  return {
    objects,
    requires
  };
}

function send_query(ws) {
  var data = query();
  // 可优化为false
  data.refresh = false;
  ws.send(JSON.stringify({
    type: "Query",
    data
  }));
}